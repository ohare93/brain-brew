import csv
import logging
import re
from enum import Enum
from typing import List, Dict

from brain_brew.representation.configuration.global_config import GlobalConfig
from brain_brew.representation.generic.yaml_file import YamlFile, ConfigKey
from brain_brew.utils import list_of_str_to_lowercase, generate_anki_guid
from brain_brew.representation.generic.generic_file import GenericFile


class CsvKeys(Enum):
    GUID = "guid"
    TAGS = "tags"


class CsvFile(GenericFile):
    file_location: str = ""
    _data: List[dict] = []

    column_headers: list = []

    def __init__(self, file, read_now=True, data_override=None):
        super(CsvFile, self).__init__(file, read_now=read_now, data_override=data_override)

    def read_file(self):
        self._data = []

        with open(self.file_location, mode='r') as csv_file:
            csv_reader = csv.DictReader(csv_file)

            self.column_headers = list_of_str_to_lowercase(csv_reader.fieldnames)

            for row in csv_reader:
                self._data.append({key.lower(): row[key] for key in row})

        self.data_state = GenericFile.DataState.READ_IN_DATA

    def write_file(self):
        with open(self.file_location, mode='w') as csv_file:
            csv_writer = csv.DictWriter(csv_file, fieldnames=self.column_headers)

            csv_writer.writeheader()

            for row in self._data:
                csv_writer.writerow(row)

        self.file_exists = True

    def set_data_from_superset(self, superset: List[dict], column_header_override=None):
        if column_header_override:
            self.column_headers = column_header_override

        data_to_set: List[dict] = []
        for row in superset:
            if not all(column in row for column in self.column_headers):
                continue
            new_row = {}
            for column in self.column_headers:
                new_row[column] = row[column]
            data_to_set.append(new_row)

        super().set_data(data_to_set)

    def get_data(self, deep_copy=False) -> List[dict]:
        return super().get_data(deep_copy=deep_copy)

    @staticmethod
    def to_filename_csv(filename: str) -> str:
        converted = re.sub(r'\s+', '-', filename).strip()
        return converted + ".csv" if not converted.endswith(".csv") else converted

    @classmethod
    def formatted_file_location(cls, location):
        return cls.to_filename_csv(location)

    def sort_data(self, sort_by_keys, reverse_sort):
        self._data = self._sort_data(self._data, sort_by_keys, reverse_sort)
