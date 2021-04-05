from dataclasses import dataclass, field
from enum import Enum
from typing import List, Union, Dict, Optional

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
            columns_to_fields: map(str(), key=str(), required=False)
            personal_fields: list(str(), required=False)
        '''

    @dataclass
    class Representation(RepresentationBase):
        note_models: Union[str, list]
        columns_to_fields: Optional[Dict[str, str]] = field(default=None)
        personal_fields: List[str] = field(default_factory=lambda: [])

    note_models: Dict[str, PartHolder[NoteModel]]
    columns_manually_mapped: List[FieldMapping]
    personal_fields: List[FieldMapping]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        note_models = [PartHolder.from_file_manager(model) for model in single_item_to_list(rep.note_models)]

        return cls(
            columns_manually_mapped=[FieldMapping(
                field_type=FieldMapping.FieldMappingType.COLUMN,
                field_name=f,
                value=key) for key, f in rep.columns_to_fields.items()]
            if rep.columns_to_fields else [],
            personal_fields=[FieldMapping(
                field_type=FieldMapping.FieldMappingType.PERSONAL_FIELD,
                field_name=f,
                value="") for f in rep.personal_fields],
            note_models=dict(map(lambda nm: (nm.part_id, nm), note_models))
        )

    def get_note_model_mapping_dict(self):
        return {model: self for model in self.note_models}

    def verify_contents(self):
        if not self.columns_manually_mapped:  # No check needed if no manual mapping is performed
            return

        errors = []
        required_field_definitions = [GUID, TAGS]

        extra_fields = [field.field_name for field in self.columns_manually_mapped
                        if field.field_name not in required_field_definitions]

        for holder in self.note_models.values():
            model: NoteModel = holder.part

            # Check for Required Fields
            missing = []
            for req in required_field_definitions:
                if req not in [field.field_name for field in self.columns_manually_mapped]:
                    missing.append(req)

            if missing:
                errors.append(KeyError(f"""Error in note_model_mappings part with note model "{holder.part_id}". \
                                           When mapping columns_to_fields you must map all fields. \
                                           Mapping is missing for for fields: {missing}"""))

            # Check Fields Align with Note Type
            missing = model.check_field_overlap(
                [field.field_name for field in self.columns_manually_mapped
                 if field.field_name not in required_field_definitions]
            )
            missing = [m for m in missing if m not in [field.field_name for field in self.personal_fields]]

            if missing:
                errors.append(KeyError(f"""Error in note_model_mappings part with note model "{holder.part_id}". \
                                           When mapping columns_to_fields you must map all fields. \
                                           Mapping is missing for for fields: {missing}"""))

            # Find mappings which do not exist on any note models
            if extra_fields:
                extra_fields = model.check_field_extra(extra_fields)

        if extra_fields:
            errors.append(
                KeyError(f"""Error in note_model_mappings part. \
                             Field(s) '{extra_fields}' are defined as mappings, but match no Note Model fields"""))

        if errors:
            raise Exception(errors)

    def csv_row_map_to_note_fields(self, row: dict) -> dict:
        relevant_row_data = self.filter_data_row_by_relevant_columns(row)

        for pf in self.personal_fields:  # Add in Personal Fields
            relevant_row_data.setdefault(pf.field_name, False)
        for column in self.columns_manually_mapped:  # Rename from Csv Column to Note Type Field
            if column.value in relevant_row_data:
                relevant_row_data[column.field_name] = relevant_row_data.pop(column.value)

        return relevant_row_data

    def csv_headers_map_to_note_fields(self, row: list) -> list:
        return list(self.csv_row_map_to_note_fields({row_name: "" for row_name in row}).keys())

    def note_fields_map_to_csv_row(self, row):
        for column in self.columns_manually_mapped:  # Rename from Note Type Field to Csv Column
            if column.field_name in row:
                row[column.value] = row.pop(column.field_name)
        for pf in self.personal_fields:  # Remove Personal Fields
            if pf.field_name in row:
                del row[pf.field_name]

        relevant_row_data = self.filter_data_row_by_relevant_columns(row)

        return relevant_row_data

    def filter_data_row_by_relevant_columns(self, row):
        cols = list(row.keys())

        relevant_columns = [f.value for f in self.columns_manually_mapped]
        if not relevant_columns:
            return row

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
