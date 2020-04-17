import logging
from enum import Enum
from typing import Dict, List

from brain_brew.representation.generic.csv_file import CsvFile, CsvKeys
from brain_brew.representation.generic.yaml_file import YamlFile, ConfigKey
from brain_brew.representation.json.deck_part_notemodel import DeckPartNoteModel
from brain_brew.utils import single_item_to_list, generate_anki_guid, list_of_str_to_lowercase


class CsvFileMappingKeys(Enum):
    CSV_FILE = "csv"
    NOTE_MODEL = "note_model"
    SORT_BY_COLUMNS = "sort_by_columns"
    REVERSE_SORT = "reverse_sort"
    DERIVATIVES = "derivatives"


class CsvFileMapping(YamlFile):
    config_entry = {}
    expected_keys = {
        CsvFileMappingKeys.CSV_FILE.value: ConfigKey(True, str, None),
        CsvFileMappingKeys.NOTE_MODEL.value: ConfigKey(True, str, None),
        CsvFileMappingKeys.SORT_BY_COLUMNS.value: ConfigKey(False, (list, str), None),
        CsvFileMappingKeys.REVERSE_SORT.value: ConfigKey(False, bool, None),
        CsvFileMappingKeys.DERIVATIVES.value: ConfigKey(False, list, None),
    }
    subconfig_filter = None

    csv_file: CsvFile
    compiled_data: Dict[str, dict]

    sort_by_columns: list
    reverse_sort: bool

    note_model: DeckPartNoteModel
    derivatives: List['CsvFileMapping']

    parent_mapping: 'CsvFileMapping'

    data_has_changed: bool

    def __init__(self, config_data, read_now=True):
        self.setup_config_with_subconfig_replacement(config_data)
        self.verify_config_entry()

        self.csv_file = CsvFile.create(self.get_config(CsvFileMappingKeys.CSV_FILE), read_now=read_now)

        self.sort_by_columns = single_item_to_list(self.get_config(CsvFileMappingKeys.SORT_BY_COLUMNS, []))
        self.reverse_sort = self.get_config(CsvFileMappingKeys.REVERSE_SORT, False)

        self.note_model = DeckPartNoteModel.create(self.get_config(CsvFileMappingKeys.NOTE_MODEL), read_now=read_now)
        self.derivatives = [CsvFileMapping(config, read_now=read_now)
                            for config in self.get_config(CsvFileMappingKeys.DERIVATIVES, [])]

    def get_available_columns(self):
        return self.csv_file.column_headers + self.get_derivative_columns()

    def get_derivative_columns(self):
        return [der.get_available_columns() for der in self.derivatives]

    def get_data(self) -> Dict[str, dict]:
        self.compiled_data = {}
        guids_generated = 0
        self.data_has_changed = False

        data_in_progress = self.build_data()

        # Fill in Guid if no Guid
        for row in data_in_progress:
            guid = row[CsvKeys.GUID.value]
            if not guid:
                guid = row[CsvKeys.GUID.value] = generate_anki_guid()
                guids_generated += 1
            self.compiled_data.setdefault(guid, {key.lower(): row[key] for key in row})

        if guids_generated > 0:
            self.data_has_changed = True
            logging.info(f"Generated {guids_generated} guids in {self.csv_file.file_location}")
        
        return self.compiled_data

    def build_data(self) -> List[dict]:
        data_in_progress = self.csv_file.get_data(deep_copy=True)

        new_columns_seen_so_far = self.csv_file.column_headers
        for der in self.derivatives:
            der_cols = der.get_available_columns()
            overlapping_cols = [col for col in der_cols if col in self.csv_file.column_headers]
            der_cols = [col for col in der_cols if col not in overlapping_cols]

            if not overlapping_cols:
                raise KeyError("No column overlap for derivative")

            column_repeat_errors = [KeyError(f"Derivative column {c} in multiple derivative lines")
                                    for c in der_cols if c in new_columns_seen_so_far]
            if column_repeat_errors:
                raise Exception(column_repeat_errors)
            new_columns_seen_so_far += der_cols

            der_match_errors = []
            for der_row in der.build_data():
                found_match = False
                for row in data_in_progress:
                    if all([der_row[c] == row[c] for c in overlapping_cols]):
                        for der_col in der_cols:
                            row[der_col] = der_row[der_col]
                        found_match = True
                        break
                if not found_match:
                    der_match_errors.append(ValueError(f"Cannot match derivative row {der_row} to parent"))

            if der_match_errors:
                raise Exception(der_match_errors)

        return data_in_progress

    def set_relevant_data(self, data_set: Dict[str, dict]):
        unchanged, changed, added = 0, 0, 0
        for guid in data_set:
            if guid in self.compiled_data.keys():
                changed_row = False
                for key in data_set[guid]:
                    if self.compiled_data[guid][key] != data_set[guid][key]:
                        self.compiled_data[guid][key] = data_set[guid][key]
                        changed_row = True
                if changed_row:
                    changed += 1
                else:
                    unchanged += 1
            else:
                added += 1
                self.compiled_data.setdefault(guid, data_set[guid])

        if changed > 0 or added > 0:
            pass
            # TODO: Call derivatives
        logging.info(f"Set {self.csv_file.file_location} data; changed {changed}, added {added}, while {unchanged} were identical")

    def get_relevant_data(self, relevant_columns: List[str]):
        if not relevant_columns:
            return []

        cols = self.get_available_columns()
        relevant_columns = list_of_str_to_lowercase(relevant_columns)

        errors = [KeyError(f"Missing column {rel_col}") for rel_col in relevant_columns if rel_col not in cols]
        if errors:
            raise Exception(errors)

        irrelevant_columns = [column for column in cols if column not in relevant_columns]
        if not irrelevant_columns:
            return self.compiled_data

        relevant_data = {}
        for guid in self.compiled_data:
            relevant_data.setdefault(guid, {key: self.compiled_data[guid][key] for key in self.compiled_data[guid]
                                            if key not in irrelevant_columns})

        return relevant_data
