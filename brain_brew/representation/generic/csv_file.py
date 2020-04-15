import csv
import logging
import re
from enum import Enum
from typing import List, Dict

from brain_brew.utils import list_of_str_to_lowercase, generate_anki_guid
from brain_brew.representation.generic.generic_file import GenericFile


class CsvKeys(Enum):
    GUID = "guid"
    TAGS = "tags"


class CsvFile(GenericFile):
    file_location: str = ""
    _data: Dict[str, dict] = {}

    column_headers: list = []

    def __init__(self, file, read_now=True, data_override=None):
        super(CsvFile, self).__init__(file, read_now=read_now, data_override=data_override)

    def read_file(self):
        self._data = {}
        guids_generated = 0

        with open(self.file_location, mode='r') as csv_file:
            csv_reader = csv.DictReader(csv_file)

            self.column_headers = list_of_str_to_lowercase(csv_reader.fieldnames)

            for row in csv_reader:
                guid = row[CsvKeys.GUID.value]
                if not guid:
                    guid = row[CsvKeys.GUID.value] = generate_anki_guid()
                    guids_generated += 1
                self._data.setdefault(guid, {key.lower(): row[key] for key in row})

        if guids_generated > 0:
            self.data_state = GenericFile.DataState.DATA_SET
            logging.info(f"Generated {guids_generated} guids in {self.file_location}")
        else:
            self.data_state = GenericFile.DataState.READ_IN_DATA

    def write_file(self):
        with open(self.file_location, mode='w') as csv_file:
            csv_writer = csv.DictWriter(csv_file, fieldnames=self.column_headers)

            csv_writer.writeheader()

            for row in self._data.values():
                csv_writer.writerow(row)

        self.file_exists = True

    def set_data(self, data_override: Dict[str, dict]):
        super().set_data(data_override)
        any_entry = next(iter(data_override.values()))
        self.column_headers = list(any_entry.keys()) if data_override else []

    def set_relevant_data(self, data_set: Dict[str, dict]):
        unchanged, changed, added = 0, 0, 0
        for guid in data_set:
            if guid in self._data.keys():
                changed_row = False
                for key in data_set[guid]:
                    if self._data[guid][key] != data_set[guid][key]:
                        self._data[guid][key] = data_set[guid][key]
                        changed_row = True
                if changed_row:
                    changed += 1
                else:
                    unchanged += 1
            else:
                added += 1
                self._data.setdefault(guid, data_set[guid])

        if changed > 0 or added > 0:
            self.data_state = GenericFile.DataState.DATA_SET
        logging.info(f"Set {self.file_location} data; changed {changed}, added {added}, while {unchanged} were identical")

    def get_data(self, deep_copy=False) -> Dict[str, dict]:
        return super().get_data(deep_copy=deep_copy)

    def get_relevant_data(self, relevant_columns: List[str]):
        if not relevant_columns:
            return []

        # TODO: check if any relevant_columns are not in the csv, raise error

        relevant_columns = list_of_str_to_lowercase(relevant_columns)
        irrelevant_columns = []

        for column in self.column_headers:
            if column not in relevant_columns:
                irrelevant_columns.append(column)

        if not irrelevant_columns:
            return self._data

        relevant_data = {}
        for guid in self._data:
            relevant_data.setdefault(guid, {key: self._data[guid][key] for key in self._data[guid]
                                            if key not in irrelevant_columns})

        return relevant_data

    @staticmethod
    def to_filename_csv(filename: str) -> str:
        converted = re.sub(r'\s+', '-', filename).strip()
        return converted + ".csv" if not converted.endswith(".csv") else converted

    @classmethod
    def formatted_file_location(cls, location):
        return cls.to_filename_csv(location)

    def sort_data(self, sort_by_keys, reverse_sort, case_insensitive_sort=None):

        sorted = self._sort_data(list(self._data.values()), sort_by_keys, reverse_sort, case_insensitive_sort)

        self._data = {row[CsvKeys.GUID.value]: {key: row[key] for key in row} for row in sorted}
