import json
import re

from brain_brew.representation.generic.generic_file import GenericFile

_encoding = "utf-8"


class JsonFile(GenericFile):
    _data: dict = {}

    def __init__(self, file, read_now=True, data_override=None):
        super(JsonFile, self).__init__(file, read_now=read_now, data_override=data_override)

    def set_data(self, data_override):
        super().set_data(data_override)

    def get_data(self, deep_copy: bool = False) -> dict:
        return super().get_data(deep_copy=deep_copy)

    def pretty_print(self):
        return json.dumps(self._data, indent=4)

    def read_file(self):
        with open(self.file_location, "r", encoding=_encoding) as read_file:
            self._data = json.load(read_file)

    def write_file(self, data_override=None):
        with open(self.file_location, "w", encoding=_encoding) as write_file:
            json.dump(data_override or self._data, write_file, indent=4, sort_keys=False, ensure_ascii=False)

        self.file_exists = True

    @staticmethod
    def to_filename_json(filename: str) -> str:
        converted = re.sub(r'\s+', '-', filename).strip()
        return converted + ".json" if not converted.endswith(".json") else converted

    @staticmethod
    def get_json_file_location(prepend, location):
        return JsonFile.to_filename_json(prepend + location)

    @classmethod
    def formatted_file_location(cls, location):
        return cls.to_filename_json(location)
