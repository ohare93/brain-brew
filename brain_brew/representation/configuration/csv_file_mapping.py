import logging
from enum import Enum
from typing import Dict, List

from brain_brew.representation.generic.csv_file import CsvFile, CsvKeys
from brain_brew.representation.generic.yaml_file import YamlFile, ConfigKey
from brain_brew.representation.json.deck_part_notemodel import DeckPartNoteModel
from brain_brew.utils import single_item_to_list, generate_anki_guid


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
    _csv_data: {}

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
        return self.csv_file.column_headers + [der.get_available_columns() for der in self.derivatives]

    def read_file(self):
        self._csv_data = {}
        guids_generated = 0
        self.data_has_changed = False

        for row in self.csv_file.get_data(deep_copy=True):
            guid = row[CsvKeys.GUID.value]
            if not guid:
                guid = row[CsvKeys.GUID.value] = generate_anki_guid()
                guids_generated += 1
            self._csv_data.setdefault(guid, {key.lower(): row[key] for key in row})

        if guids_generated > 0:
            self.data_has_changed = True
            logging.info(f"Generated {guids_generated} guids in {self.csv_file.file_location}")

        # TODO: Do stuff with derivatives

    def set_relevant_data(self, data_set: Dict[str, dict]):
        unchanged, changed, added = 0, 0, 0
        for guid in data_set:
            if guid in self._csv_data.keys():
                changed_row = False
                for key in data_set[guid]:
                    if self._csv_data[guid][key] != data_set[guid][key]:
                        self._csv_data[guid][key] = data_set[guid][key]
                        changed_row = True
                if changed_row:
                    changed += 1
                else:
                    unchanged += 1
            else:
                added += 1
                self._csv_data.setdefault(guid, data_set[guid])

        if changed > 0 or added > 0:
            pass
            # TODO: Call derivatives
        logging.info(f"Set {self.csv_file.file_location} data; changed {changed}, added {added}, while {unchanged} were identical")