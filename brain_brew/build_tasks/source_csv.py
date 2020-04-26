from enum import Enum
from typing import List, Dict

from brain_brew.build_tasks.build_task_generic import BuildTaskGeneric
from brain_brew.constants.build_config_keys import BuildTaskEnum, BuildConfigKeys
from brain_brew.constants.deckpart_keys import DeckPartNoteKeys
from brain_brew.interfaces.verifiable import Verifiable
from brain_brew.representation.configuration.csv_file_mapping import CsvFileMapping
from brain_brew.representation.configuration.note_model_mapping import NoteModelMapping
from brain_brew.utils import single_item_to_list
from brain_brew.representation.generic.yaml_file import ConfigKey, YamlFile
from brain_brew.representation.generic.csv_file import CsvFile, CsvKeys
from brain_brew.representation.json.deck_part_notemodel import DeckPartNoteModel
from brain_brew.representation.json.deck_part_notes import DeckPartNotes


class SourceCsvKeys(Enum):
    NOTES = "notes"
    NOTE_MODEL_MAPPINGS = "note_model_mappings"
    CSV_MAPPINGS = "csv_file_mappings"


class SourceCsv(YamlFile, BuildTaskGeneric, Verifiable):
    @staticmethod
    def get_build_keys():
        return [
            BuildTaskEnum("deck_parts_to_csv_collection", SourceCsv, "deck_parts_to_source", "source_to_deck_parts"),
            BuildTaskEnum("csv_collection_to_deck_parts", SourceCsv, "source_to_deck_parts", "deck_parts_to_source"),
        ]

    config_entry = {}
    expected_keys = {
        SourceCsvKeys.NOTES.value: ConfigKey(True, str, None),
        SourceCsvKeys.NOTE_MODEL_MAPPINGS.value: ConfigKey(True, list, None),
        SourceCsvKeys.CSV_MAPPINGS.value: ConfigKey(True, list, None),
    }
    subconfig_filter = None

    notes: DeckPartNotes
    note_model_mappings_dict: Dict[str, NoteModelMapping]
    csv_file_mappings: List[CsvFileMapping]

    def __init__(self, config_data: dict, read_now=True):
        self.setup_config_with_subconfig_replacement(config_data)
        self.verify_config_entry()

        self.notes = DeckPartNotes.create(self.get_config(BuildConfigKeys.NOTES), read_now=read_now)

        nm_mapping = [NoteModelMapping(config, read_now=read_now)
                                                         for config in
                                                         self.get_config(SourceCsvKeys.NOTE_MODEL_MAPPINGS)
                                                         ]
        self.note_model_mappings_dict = {mapping.note_model.name: mapping
                                         for mapping in nm_mapping}

        self.csv_file_mappings = [CsvFileMapping(config, read_now=read_now)
                                  for config in self.get_config(SourceCsvKeys.CSV_MAPPINGS)]

    @classmethod
    def from_yaml(cls, yaml_file_name, read_now=True):
        config_data = YamlFile.read_file(yaml_file_name)

        return SourceCsv(config_data, read_now=read_now)

    def verify_contents(self):
        errors = []

        for nm in self.note_model_mappings_dict.values():
            try:
                nm.verify_contents()
            except KeyError as e:
                errors.append(e)

        for cfm in self.csv_file_mappings:
            # Check all necessary key values are present
            try:
                cfm.verify_contents()
            except KeyError as e:
                errors.append(e)

            # Check all references notemodels have a mapping
            for csv_map in self.csv_file_mappings:
                for nm in csv_map.get_used_note_model_names():
                    if nm not in self.note_model_mappings_dict.keys():
                        errors.append(f"Missing Note Model Map for {nm}")

        # Check each of the Csvs (or their derivatives) contain all the necessary columns for their stated note model
        for cfm in self.csv_file_mappings:
            note_model_names = cfm.get_used_note_model_names()
            available_columns = cfm.get_available_columns()

            referenced_note_models_maps = [value for key, value in self.note_model_mappings_dict.items() if
                                           key in note_model_names]
            for nm_map in referenced_note_models_maps:
                missing_columns = [col for col in nm_map.note_model.fields_lowercase if
                                   col not in nm_map.csv_headers_map_to_note_fields(available_columns)]
                if missing_columns:
                    errors.append(KeyError(f"Csvs are missing columns from {nm_map.note_model.name}", missing_columns))

        if errors:
            raise Exception(errors)

    def notes_to_deck_parts(self):
        csv_data_by_guid: Dict[str, dict] = {}
        for csv_map in self.csv_file_mappings:
            csv_map.compile_data()
            csv_data_by_guid = {**csv_data_by_guid, **csv_map.compiled_data}
        csv_rows: List[dict] = list(csv_data_by_guid.values())

        notes_json = []
        top_level_note_structure = {
            DeckPartNoteKeys.FIELDS.value: List[str],
            DeckPartNoteKeys.GUID.value: "",
            DeckPartNoteKeys.TAGS.value: List[str],
            DeckPartNoteKeys.NOTE_MODEL.value: ""
        }

        # Get Guid, Tags, NoteTypeName, Fields
        for row in csv_rows:
            note = top_level_note_structure.copy()

            row_nm: NoteModelMapping = self.note_model_mappings_dict[row[DeckPartNoteKeys.NOTE_MODEL.value]]

            filtered_fields = row_nm.csv_row_map_to_note_fields(row)

            note[DeckPartNoteKeys.NOTE_MODEL.value] = row_nm.note_model.name
            note[DeckPartNoteKeys.GUID.value] = filtered_fields.pop(DeckPartNoteKeys.GUID.value)
            note[DeckPartNoteKeys.TAGS.value] = self.split_tags(filtered_fields.pop(DeckPartNoteKeys.TAGS.value))

            note[DeckPartNoteKeys.FIELDS.value] = row_nm.field_values_in_note_model_order(filtered_fields)

            notes_json.append(note)

        return notes_json

    def notes_to_source(self) -> Dict[str, dict]:
        notes_data = self.notes.get_data(deep_copy=True)[DeckPartNoteKeys.NOTES.value]
        self.verify_notes_match_note_model_mappings(notes_data)

        csv_data: Dict[str, dict] = {}
        for note in notes_data:
            nm_name = note[DeckPartNoteKeys.NOTE_MODEL.value]
            row = self.note_model_mappings_dict[nm_name].note_model.zip_field_to_data(
                note[DeckPartNoteKeys.FIELDS.value])
            row[CsvKeys.GUID.value] = note[DeckPartNoteKeys.GUID.value]
            row[CsvKeys.TAGS.value] = self.join_tags(note[DeckPartNoteKeys.TAGS.value])

            formatted_row = self.note_model_mappings_dict[nm_name].note_fields_map_to_csv_row(row)

            csv_data.setdefault(row[CsvKeys.GUID.value], formatted_row)

        return csv_data

    def verify_notes_match_note_model_mappings(self, notes):
        note_models_used = {note[DeckPartNoteKeys.NOTE_MODEL.value] for note in notes}
        errors = [TypeError(f"Unknown note model type '{model}' in notes '{self.notes.file_location}. "
                            f"Add {SourceCsvKeys.NOTE_MODEL_MAPPINGS.value} for that model.")
                  for model in note_models_used if model not in self.note_model_mappings_dict.keys()]

        if errors:
            raise Exception(errors)

    def source_to_deck_parts(self):
        notes_data = self.notes_to_deck_parts()
        self.notes.set_data(notes_data)

    def deck_parts_to_source(self):
        csv_data = self.notes_to_source()

        for cfm in self.csv_file_mappings:
            cfm.compile_data()
            cfm.set_relevant_data(csv_data)
