from enum import Enum
from typing import List, Dict

from brain_brew.build_tasks.build_task_generic import BuildTaskGeneric
from brain_brew.constants.build_config_keys import BuildTaskEnum, BuildConfigKeys
from brain_brew.constants.deckpart_keys import DeckPartNoteKeys
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


class SourceCsv(YamlFile, BuildTaskGeneric):
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
    note_model_mappings: List[NoteModelMapping]
    csv_file_mappings: List[CsvFileMapping]

    def __init__(self, config_data: dict, read_now=True):
        self.setup_config_with_subconfig_replacement(config_data)
        self.verify_config_entry()

        self.notes = DeckPartNotes.create(self.get_config(BuildConfigKeys.NOTES), read_now=read_now)

        self.note_model_mappings = [NoteModelMapping(config, read_now=read_now)
                                    for config in self.get_config(SourceCsvKeys.NOTE_MODEL_MAPPINGS)]

        self.csv_file_mappings = [CsvFileMapping(config, read_now=read_now)
                                  for config in self.get_config(SourceCsvKeys.CSV_MAPPINGS)]

    @classmethod
    def from_yaml(cls, yaml_file_name, read_now=True):
        config_data = YamlFile.read_file(yaml_file_name)

        return SourceCsv(config_data, read_now=read_now)

    def get_data(self):
        columns = [field.field_name for field in self.columns]
        csv_data = self.csv_file.get_relevant_data(columns)

        for row in csv_data.values():
            for pf in self.personal_fields:
                row.setdefault(pf.field_name, False)
            for column in self.columns:
                row[column.value] = row.pop(column.field_name)

        # TODO: Insert FieldMappings with Default values

        return csv_data

    def check_for_required_fields(self):
        missing = []
        for req in self.required_fields_definitions:
            if req not in [field.value for field in self.columns]:
                missing.append(req)

        if missing:
            raise KeyError(f"""Note model "{self.note_model.name}" to Csv config error: \
                               Definitions for fields {missing} are required.""", self.csv_file.file_location)

    def check_fields_align_with_note_type(self):
        error_in_config = False
        # Check NoteType Columns
        missing, extra = self.note_model.check_field_overlap(
            [field.value for field in self.columns if field.value not in self.required_fields_definitions]
        )

        if missing:
            s1 = sorted([field.field_name for field in self.personal_fields])
            s2 = sorted(missing)  # TODO: remove personal fields from the error message
            if s1 != s2:
                error_in_config = True

        if extra:
            error_in_config = True

        if error_in_config:
            raise KeyError(
                f"""Note model "{self.note_model.name}" to Csv config error. It expected {self.note_model.fields} \
                    but was missing: {missing}, and got extra: {extra} """)

    def notes_to_deck_parts(self):

        self.check_for_required_fields()
        self.check_fields_align_with_note_type()

        csv_rows = self.get_data().values()

        # TODO: Derivatives
        # Check again if fields align for each possible derivative type

        for row in csv_rows:
            row.setdefault(DeckPartNoteKeys.NOTE_MODEL.value, self.note_model)

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

            row_nm: DeckPartNoteModel = row[DeckPartNoteKeys.NOTE_MODEL.value]

            note[DeckPartNoteKeys.NOTE_MODEL.value] = row_nm.name
            note[DeckPartNoteKeys.GUID.value] = row[DeckPartNoteKeys.GUID.value]
            note[DeckPartNoteKeys.TAGS.value] = self.split_tags(row[DeckPartNoteKeys.TAGS.value])

            note[DeckPartNoteKeys.FIELDS.value] = [row[field.lower()] for field in row_nm.fields]

            notes_json.append(note)

        return notes_json

    def notes_to_source(self) -> Dict[str, dict]:

        self.check_for_required_fields()
        self.check_fields_align_with_note_type()  # TODO: will begin failing when derivatives are added; add override

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
        for csv_map in self.csv_file_mappings:
            csv_map.get_data()

        notes_data = self.notes_to_deck_parts()
        self.notes.set_data(notes_data)

    def deck_parts_to_source(self):
        csv_data = self.notes_to_source()

        self.csv_file.set_relevant_data(csv_data)
        self.csv_file.sort_data(self.sort_by_columns, self.reverse_sort)  # TODO: Move
