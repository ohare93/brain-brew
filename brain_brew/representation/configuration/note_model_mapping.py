from dataclasses import dataclass
from enum import Enum
from typing import List, Union, Dict

from brain_brew.constants.deckpart_keys import DeckPartNoteKeys
from brain_brew.interfaces.verifiable import Verifiable
from brain_brew.representation.generic.yaml_file import YamlFile, ConfigKey
from brain_brew.representation.json.deck_part_notemodel import DeckPartNoteModel
from brain_brew.utils import list_of_str_to_lowercase, single_item_to_list

NOTE_MODEL = "note_model"
COLUMNS = "csv_columns_to_fields"
PERSONAL_FIELDS = "personal_fields"


class FieldMapping:
    class FieldMappingType(Enum):
        COLUMN = "column"
        PERSONAL_FIELD = "personal_field"
        DEFAULT = "default"

        @classmethod
        def values(cls):
            return set(it.value for it in cls)

    type: FieldMappingType
    value: str
    field_name: str

    def __init__(self, field_type: FieldMappingType, field_name: str, value: str):
        self.type = field_type
        self.field_name = field_name.lower()

        if self.type == FieldMapping.FieldMappingType.COLUMN:
            self.value = value.lower()
        else:
            self.value = value


@dataclass
class NoteModelMapping(Verifiable):
    @dataclass
    class Representation:
        note_models: Union[str, list]
        columns_to_fields: Dict[str, str]
        personal_fields: List[str]

        @classmethod
        def from_dict(cls, data: dict):
            return cls(**data)

    note_models: Dict[str, DeckPartNoteModel]
    columns: List[FieldMapping]
    personal_fields: List[FieldMapping]

    required_fields_definitions = [DeckPartNoteKeys.GUID.value, DeckPartNoteKeys.TAGS.value]

    @classmethod
    def from_repr(cls, data: Representation):
        note_models = [DeckPartNoteModel.create(model, read_now=True) for model in single_item_to_list(data.note_models)]

        return cls(
            columns=[FieldMapping(
                field_type=FieldMapping.FieldMappingType.COLUMN,
                field_name=field,
                value=key) for key, field in data.columns_to_fields.items()],
            personal_fields=[FieldMapping(
                field_type=FieldMapping.FieldMappingType.PERSONAL_FIELD,
                field_name=field,
                value="") for field in data.personal_fields],
            note_models=dict(map(lambda nm: (nm.name, nm), note_models))  # TODO: Use deck part pool
        )

    def verify_contents(self):
        errors = []

        for model in self.note_models:
            # Check for Required Fields
            missing = []
            for req in self.required_fields_definitions:
                if req not in [field.value for field in self.columns]:
                    missing.append(req)

            if missing:
                errors.append(KeyError(f"""Note model(s) "{model.name}" to Csv config error: \
                                   Definitions for fields {missing} are required."""))

            # TODO: Note Model Mappings are allowed extra fields on a specific note model now, since multiple
            # TODO: can be applied to a single NMM. Check if ANY are missing and/or ALL have Extra instead
            # Check Fields Align with Note Type
            missing, extra = model.check_field_overlap(
                [field.value for field in self.columns if field.value not in self.required_fields_definitions]
            )
            missing = [m for m in missing if m not in [field.field_name for field in self.personal_fields]]

            if missing or extra:
                errors.append(KeyError(
                    f"""Note model "{model.name}" to Csv config error. It expected {model.fields} \
                        but was missing: {missing}, and got extra: {extra} """))

            # TODO: Make sure the same note_model is not defined in multiple NMMs
        if errors:
            raise Exception(errors)

    def csv_row_map_to_note_fields(self, row: dict) -> dict:
        relevant_row_data = self.get_relevant_data(row)

        for pf in self.personal_fields:  # Add in Personal Fields
            relevant_row_data.setdefault(pf.field_name, False)
        for column in self.columns:  # Rename from Csv Column to Note Type Field
            relevant_row_data[column.value] = relevant_row_data.pop(column.field_name)

        # TODO: Insert FieldMappings with Default values

        return relevant_row_data

    def csv_headers_map_to_note_fields(self, row: list) -> list:
        return list(self.csv_row_map_to_note_fields({row_name: "" for row_name in row}).keys())

    def note_fields_map_to_csv_row(self, row):
        for column in self.columns:  # Rename from Note Type Field to Csv Column
            row[column.field_name] = row.pop(column.value)

        for pf in self.personal_fields:  # Remove Personal Fields
            del row[pf.field_name]

        relevant_row_data = self.get_relevant_data(row)

        return relevant_row_data

    def get_relevant_data(self, row):
        relevant_columns = [field.field_name for field in self.columns]
        if not relevant_columns:
            return []

        cols = row.keys()

        errors = [KeyError(f"Missing column {rel_col}") for rel_col in relevant_columns if rel_col not in cols]
        if errors:
            raise Exception(errors)

        irrelevant_columns = [column for column in cols if column not in relevant_columns]
        if not irrelevant_columns:
            return row

        relevant_data = {key: row[key] for key in row if key not in irrelevant_columns}

        return relevant_data

    def field_values_in_note_model_order(self, note_model_name, fields_from_csv):
        return [fields_from_csv[field] for field in self.note_models[note_model_name].fields_lowercase]
