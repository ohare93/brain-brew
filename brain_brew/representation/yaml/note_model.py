from dataclasses import dataclass, field
from typing import List

from brain_brew.representation.yaml.my_yaml import YamlRepresentation
from brain_brew.utils import list_of_str_to_lowercase


@dataclass
class DeckPartNoteModel(YamlRepresentation):
    name: str
    id: str
    fields: List[str]

    @property
    def fields_lowercase(self):
        return list_of_str_to_lowercase(self.fields)

    def check_field_overlap(self, fields_to_check: List[str]):
        fields_to_check = list_of_str_to_lowercase(fields_to_check)
        lower_fields = self.fields_lowercase

        missing = [field for field in lower_fields if field not in fields_to_check]
        extra = [field for field in fields_to_check if field not in lower_fields]

        return missing, extra

    def zip_field_to_data(self, data: List[str]) -> dict:
        if len(self.fields) != len(data):
            raise Exception(f"Data of length {len(data)} cannot map to fields of length {len(self.fields_lowercase)}")
        return dict(zip(self.fields_lowercase, data))


