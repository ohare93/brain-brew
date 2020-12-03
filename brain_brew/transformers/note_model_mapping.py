from dataclasses import dataclass
from enum import Enum
from typing import List, Union, Dict

from brain_brew.configuration.part_holder import PartHolder
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.interfaces.yamale_verifyable import YamlRepr
from brain_brew.representation.yaml.note_model import NoteModel
from brain_brew.representation.yaml.notes import GUID, TAGS
from brain_brew.utils import single_item_to_list


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
class NoteModelMapping(YamlRepr):
    @classmethod
    def task_name(cls) -> str:
        return r'note_model_mapping'

    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            note_models: any(list(str()), str())
            columns_to_fields: map(str(), key=str())
            personal_fields: list(str())
        '''

    @dataclass
    class Representation(RepresentationBase):
        note_models: Union[str, list]
        columns_to_fields: Dict[str, str]
        personal_fields: List[str]

    note_models: Dict[str, PartHolder[NoteModel]]
    columns: List[FieldMapping]
    personal_fields: List[FieldMapping]

    required_fields_definitions = [GUID, TAGS]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        note_models = [PartHolder.from_file_manager(model) for model in single_item_to_list(rep.note_models)]

        return cls(
            columns=[FieldMapping(
                field_type=FieldMapping.FieldMappingType.COLUMN,
                field_name=field,
                value=key) for key, field in rep.columns_to_fields.items()],
            personal_fields=[FieldMapping(
                field_type=FieldMapping.FieldMappingType.PERSONAL_FIELD,
                field_name=field,
                value="") for field in rep.personal_fields],
            note_models=dict(map(lambda nm: (nm.part_id, nm), note_models))
        )

    def get_note_model_mapping_dict(self):
        return {model: self for model in self.note_models}

    def verify_contents(self):
        errors = []

        extra_fields = [field.field_name for field in self.columns
                        if field.field_name not in self.required_fields_definitions]

        for holder in self.note_models.values():
            model: NoteModel = holder.part

            # Check for Required Fields
            missing = []
            for req in self.required_fields_definitions:
                if req not in [field.value for field in self.columns]:
                    missing.append(req)

            if missing:
                errors.append(KeyError(f"""Note model(s) "{holder.part_id}" to Csv config error: \
                                   Definitions for fields {missing} are required."""))

            # Check Fields Align with Note Type
            missing = model.check_field_overlap(
                [field.field_name for field in self.columns
                 if field.field_name not in self.required_fields_definitions]
            )
            missing = [m for m in missing if m not in [field.field_name for field in self.personal_fields]]

            if missing:
                errors.append(
                    KeyError(f"Note model '{holder.part_id}' to Csv config error. "
                             f"It expected {[field.name for field in model.fields]} but was missing: {missing}")
                )

            # Find mappings which do not exist on any note models
            if extra_fields:
                extra_fields = model.check_field_extra(extra_fields)

        if extra_fields:
            errors.append(
                KeyError(f"Field(s) '{extra_fields} are defined as mappings, but match no Note Model's field"))

        if errors:
            raise Exception(errors)

    def csv_row_map_to_note_fields(self, row: dict) -> dict:
        relevant_row_data = self.get_relevant_data(row)

        for pf in self.personal_fields:  # Add in Personal Fields
            relevant_row_data.setdefault(pf.field_name, False)
        for column in self.columns:  # Rename from Csv Column to Note Type Field
            if column.value in relevant_row_data:
                relevant_row_data[column.field_name] = relevant_row_data.pop(column.value)

        return relevant_row_data

    def csv_headers_map_to_note_fields(self, row: list) -> list:
        return list(self.csv_row_map_to_note_fields({row_name: "" for row_name in row}).keys())

    def note_fields_map_to_csv_row(self, row):
        for column in self.columns:  # Rename from Note Type Field to Csv Column
            if column.field_name in row:
                row[column.value] = row.pop(column.field_name)

        for pf in self.personal_fields:  # Remove Personal Fields
            if pf.field_name in row:
                del row[pf.field_name]

        relevant_row_data = self.get_relevant_data(row)

        return relevant_row_data

    def get_relevant_data(self, row):
        relevant_columns = [field.value for field in self.columns]
        if not relevant_columns:
            return []

        cols = list(row.keys())

        # errors = [KeyError(f"Missing column {rel_col}") for rel_col in relevant_columns if rel_col not in cols]
        # if errors:
        #     raise Exception(errors)

        irrelevant_columns = [column for column in cols if column not in relevant_columns]
        if not irrelevant_columns:
            return row

        relevant_data = {key: row[key] for key in row if key not in irrelevant_columns}

        return relevant_data

    def field_values_in_note_model_order(self, note_model_name, fields_from_csv):
        return [fields_from_csv[f] if f in fields_from_csv else ""
                for f in self.note_models[note_model_name].part.field_names_lowercase
                ]
