import csv
from pathlib import Path
import re
import logging
from enum import Enum
from typing import List, Optional

from brain_brew.representation.generic.source_file import SourceFile
from brain_brew.utils import create_path_if_not_exists, list_of_str_to_lowercase, sort_dict

_encoding = "utf-8"


class CsvKeys(Enum):
    GUID = "guid"
    TAGS = "tags"


class CsvFile(SourceFile):
    file_location: str = ""
    _data: List[dict] = []
    column_headers: list = []
    delimiter: str = ','

    def __init__(self, file, delimiter=None):
        self.file_location = file
        self.set_delimiter(delimiter)

    def set_delimiter(self, delimiter: str):
        if delimiter:
            self.delimiter = delimiter
        elif re.match(r'.*\.tsv', self.file_location, re.RegexFlag.IGNORECASE):
            self.delimiter = '\t'

    @classmethod
    def from_file_loc(cls, file_loc) -> 'CsvFile':
        return cls(file_loc)

    def read_file(self, create_if_not_exists: Optional[bool] = True):
        self._data = []

        if create_if_not_exists:
            create_path_if_not_exists(self.file_location)
            Path(self.file_location).touch()

        with open(self.file_location, mode='r', newline='', encoding=_encoding) as csv_file:
            csv_reader = csv.DictReader(csv_file, delimiter=self.delimiter)

            self.column_headers = list_of_str_to_lowercase(csv_reader.fieldnames)

            for row in csv_reader:
                self._data.append({key.lower(): row[key] for key in row})

    def write_file(self):
        logging.info(f"Writing to Csv '{self.file_location}'")
        with open(self.file_location, mode='w+', newline='', encoding=_encoding) as csv_file:
            csv_writer = csv.DictWriter(csv_file, fieldnames=self.column_headers, lineterminator='\n', delimiter=self.delimiter)

            csv_writer.writeheader()

            for row in self._data:
                csv_writer.writerow(row)

    def set_data(self, data_override):
        self._data = data_override
        self.column_headers = list(data_override[0].keys()) if data_override else []

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
        
        self._data = data_to_set


    def get_data(self, deep_copy=False) -> List[dict]:
        return self.get_deep_copy(self._data) if deep_copy else self._data

    @staticmethod
    def to_filename_csv(filename: str, delimiter: str = None) -> str:
        if not re.match(r'.*\.(csv|tsv)', filename, re.RegexFlag.IGNORECASE):
            if delimiter == '\t':
                return filename + '.tsv'
            else:
                return filename + ".csv"
        return filename

    @classmethod
    def formatted_file_location(cls, location):
        return cls.to_filename_csv(location)

    def sort_data(self, sort_by_keys, reverse_sort, case_insensitive_sort):
        self._data = sort_dict(self._data, sort_by_keys, reverse_sort, case_insensitive_sort)

    @classmethod
    def create_file_with_headers(cls, filepath: str, headers: List[str], delimiter: str = None):
        with open(filepath, mode='w+', newline='', encoding=_encoding) as csv_file:
            csv_writer = csv.DictWriter(csv_file, fieldnames=headers, lineterminator='\n', delimiter=delimiter or ",")

            csv_writer.writeheader()

    @staticmethod
    def delimiter_matches_file_type(delimiter: str, filename: str) -> bool:
        if delimiter == '\t' and re.match(r'.*\.tsv', filename, re.RegexFlag.IGNORECASE):
            return True
        if delimiter == ',' and re.match(r'.*\.csv', filename, re.RegexFlag.IGNORECASE):
            return True
        return False
