import csv
import re
from typing import List

from brain_brew.helper.helperfunctions import list_of_str_to_lowercase
from brain_brew.representation.generic.generic_file import GenericFile


class CsvFile(GenericFile):
    file_location: str = ""
    _data: list = []

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

    def set_data(self, data_override):
        super().set_data(data_override)
        self.column_headers = list(data_override[0].keys()) if data_override else []

    def set_relevant_data(self, data_set):
        known_guids = {guid for guid in self._data}

    def get_data(self):
        return self._data

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

        relevant_data = []
        for row in self._data:
            relevant_data.append({key: row[key] for key in row if key not in irrelevant_columns})

        return relevant_data

    @staticmethod
    def to_filename_csv(filename: str) -> str:
        converted = re.sub(r'\s+', '-', filename).strip()
        return converted + ".csv" if not converted.endswith(".csv") else converted

    @classmethod
    def formatted_file_location(cls, location):
        return cls.to_filename_csv(location)
