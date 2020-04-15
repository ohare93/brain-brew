from enum import Enum
from typing import List

from brain_brew.utils import list_of_str_to_lowercase
from brain_brew.representation.configuration.global_config import GlobalConfig
from brain_brew.representation.json.json_file import JsonFile


class CANoteModelKeys(Enum):
    ID = "crowdanki_uuid"
    NAME = "name"
    FIELDS = "flds"


class DeckPartNoteModel(JsonFile):
    name: str
    id: str
    fields: List[str]

    @classmethod
    def formatted_file_location(cls, location):
        return cls.get_json_file_location(GlobalConfig.get_instance().deck_parts.note_models, location)

    def __init__(self, location, read_now=True, data_override=None):
        super().__init__(
            self.formatted_file_location(location),
            read_now=read_now, data_override=data_override
        )

        if read_now or data_override:
            self.name = self._data[CANoteModelKeys.NAME.value]
            self.id = self._data[CANoteModelKeys.ID.value]
            self.fields = self.read_fields()

    def read_fields(self) -> List[str]:
        return [field[CANoteModelKeys.NAME.value] for field in self._data[CANoteModelKeys.FIELDS.value]]

    def check_field_overlap(self, fields_to_check: List[str]):
        fields_to_check = list_of_str_to_lowercase(fields_to_check)
        lower_fields = list_of_str_to_lowercase(self.fields)

        missing = [field for field in lower_fields if field not in fields_to_check]
        extra = [field for field in fields_to_check if field not in lower_fields]

        return missing, extra

    def zip_field_to_data(self, data: List[str]) -> dict:
        if len(self.fields) != len(data):
            raise Exception(f"Data of length {len(data)} cannot map to fields of length {len(self.fields)}")
        return dict(zip(list_of_str_to_lowercase(self.fields), data))
