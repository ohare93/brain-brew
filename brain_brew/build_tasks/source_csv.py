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
    note_model_mappings: Dict[str, NoteModelMapping]
    csv_file_mappings: List[CsvFileMapping]

    def __init__(self, config_data: dict, read_now=True):
        self.setup_config_with_subconfig_replacement(config_data)
        self.verify_config_entry()

        self.notes = DeckPartNotes.create(self.get_config(BuildConfigKeys.NOTES), read_now=read_now)

        nm_mappings = [NoteModelMapping(config, read_now=read_now)
                                    for config in self.get_config(SourceCsvKeys.NOTE_MODEL_MAPPINGS)]
        self.note_model_mappings = {mapping.note_model.name: mapping for mapping in nm_mappings}

        self.csv_file_mappings = [CsvFileMapping(config, read_now=read_now)
                                  for config in self.get_config(SourceCsvKeys.CSV_MAPPINGS)]

    @classmethod
    def from_yaml(cls, yaml_file_name, read_now=True):
        config_data = YamlFile.read_file(yaml_file_name)

        return SourceCsv(config_data, read_now=read_now)

    def verify_contents(self):
        errors = []

        for nm in self.note_model_mappings.values():
            try:
                nm.verify_contents()
            except KeyError as e:
                errors.append(e)

        for cfm in self.csv_file_mappings:
            try:
                cfm.verify_contents()
            except KeyError as e:
                errors.append(e)

        if errors:
            raise Exception(errors)

    def notes_to_deck_parts(self):
        csv_data_by_guid: Dict[str, dict] = {}
        for csv_map in self.csv_file_mappings:
            csv_data_by_guid = {**csv_data_by_guid, **csv_map.get_data()}
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

            row_nm: str = row.pop(DeckPartNoteKeys.NOTE_MODEL.value)

            note[DeckPartNoteKeys.FIELDS.value] = self.note_model_mappings[row_nm].filter_row_through_map(row)

            note[DeckPartNoteKeys.NOTE_MODEL.value] = row_nm
            note[DeckPartNoteKeys.GUID.value] = note[DeckPartNoteKeys.FIELDS.value].pop(DeckPartNoteKeys.GUID.value)
            note[DeckPartNoteKeys.TAGS.value] = self.split_tags(note[DeckPartNoteKeys.FIELDS.value].pop(DeckPartNoteKeys.TAGS.value))

            notes_json.append(note)

        return notes_json

    def notes_to_source(self) -> Dict[str, dict]:
        notes_data = self.notes.get_data(deep_copy=True)[DeckPartNoteKeys.NOTES.value]

        csv_data: Dict[str, dict] = {}
        for note in notes_data:
            if note[DeckPartNoteKeys.NOTE_MODEL.value] != self.note_model.name:
                continue
            # TODO: Add derivatives here

            row = self.note_model.zip_field_to_data(note[DeckPartNoteKeys.FIELDS.value])
            row[CsvKeys.GUID.value] = note[DeckPartNoteKeys.GUID.value]
            row[CsvKeys.TAGS.value] = self.join_tags(note[DeckPartNoteKeys.TAGS.value])

            formatted_row = {field_map.field_name: row[key]
                             for key in row.keys() for field_map in self.columns if key == field_map.value}

            csv_data.setdefault(row[CsvKeys.GUID.value], formatted_row)

        return csv_data

    def source_to_deck_parts(self):
        notes_data = self.notes_to_deck_parts()
        self.notes.set_data(notes_data)

    def deck_parts_to_source(self):
        csv_data = self.notes_to_source()

        self.csv_file.set_relevant_data(csv_data)
        self.csv_file.sort_data(self.sort_by_columns, self.reverse_sort)  # TODO: Move
