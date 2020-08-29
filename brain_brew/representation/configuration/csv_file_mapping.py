import logging
from dataclasses import dataclass, field
from enum import Enum
from typing import Dict, List, Optional, Union

from brain_brew.constants.deckpart_keys import DeckPartNoteKeys
from brain_brew.interfaces.verifiable import Verifiable
from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.representation.generic.csv_file import CsvFile, CsvKeys
from brain_brew.utils import single_item_to_list, generate_anki_guid


FILE = "csv"
NOTE_MODEL = "note_model"
SORT_BY_COLUMNS = "sort_by_columns"
REVERSE_SORT = "reverse_sort"
DERIVATIVES = "derivatives"


@dataclass
class FileMappingDerivative:
    @dataclass(init=False)
    class Representation(RepresentationBase):
        file: str
        note_model: Optional[str]
        sort_by_columns: Optional[Union[list, str]]
        reverse_sort: Optional[bool]
        derivatives: Optional[List['FileMappingDerivative.Representation']]

        def __init__(self, file, note_model=None, sort_by_columns=None, reverse_sort=None, derivatives=None):
            self.file = file
            self.note_model = note_model
            self.sort_by_columns = sort_by_columns
            self.reverse_sort = reverse_sort
            self.derivatives = list(map(FileMappingDerivative.Representation.from_dict, derivatives)) if derivatives is not None else []

    compiled_data: Dict[str, dict] = field(init=False)

    csv_file: CsvFile

    note_model: Optional[str]
    sort_by_columns: Optional[list]
    reverse_sort: Optional[bool]
    derivatives: Optional[List['FileMappingDerivative']]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            csv_file=CsvFile.create_or_get(rep.file),
            note_model=None if not rep.note_model.strip() else rep.note_model.strip(),
            sort_by_columns=single_item_to_list(rep.sort_by_columns),
            reverse_sort=rep.reverse_sort or False,
            derivatives=list(map(cls.from_repr, rep.derivatives)) if rep.derivatives is not None else []
        )

    def get_available_columns(self):
        return self.csv_file.column_headers + [col for der in self.derivatives for col in der.get_available_columns()]

    def get_used_note_model_names(self) -> List[str]:
        nm = [self.note_model] if self.note_model is not None else []
        return nm + [name for der in self.derivatives for name in der.get_used_note_model_names()]

    def _build_data_recursive(self) -> List[dict]:
        data_in_progress = self.csv_file.get_data(deep_copy=True)

        new_columns_seen_so_far = self.csv_file.column_headers.copy()
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
            for der_row in der._build_data_recursive():
                # Find matching row to pair data with
                found_match = False
                for row in data_in_progress:
                    if all([der_row[c] == row[c] for c in overlapping_cols]):
                        for der_col in der_cols:
                            row[der_col] = der_row[der_col]
                        found_match = True
                        # Set Note Model to matching Derivative Note Model
                        if der.note_model is not None:
                            row.setdefault(DeckPartNoteKeys.NOTE_MODEL.value, der.note_model)
                        break
                if not found_match:
                    der_match_errors.append(ValueError(f"Cannot match derivative row {der_row} to parent"))

            if der_match_errors:
                raise Exception(der_match_errors)

        return data_in_progress

    def write_to_csv(self, data_to_set):
        self.csv_file.set_data_from_superset(data_to_set)
        self.csv_file.sort_data(self.sort_by_columns, self.reverse_sort)

        for der in self.derivatives:
            der.write_to_csv(data_to_set)


@dataclass
class FileMapping(FileMappingDerivative, Verifiable):
    note_model: str  # Override Optional on Parent

    data_set_has_changed: bool = field(init=False, default=False)

    def verify_contents(self):
        if self.note_model is "":
            raise KeyError(f"Top level Csv Mapping requires key {NOTE_MODEL}")

    def compile_data(self):
        self.compiled_data = {}
        self.data_set_has_changed = False

        data_in_progress = self._build_data_recursive()

        # Set Note Model if not already set
        if self.note_model is not None:
            for row in data_in_progress:
                row.setdefault(DeckPartNoteKeys.NOTE_MODEL.value, self.note_model)

        # Fill in Guid if no Guid
        guids_generated = 0
        for row in data_in_progress:
            guid = row[CsvKeys.GUID.value]
            if not guid:
                guid = row[CsvKeys.GUID.value] = generate_anki_guid()
                guids_generated += 1
            self.compiled_data.setdefault(guid, {key.lower(): row[key] for key in row})

        if guids_generated > 0:
            self.data_set_has_changed = True
            logging.info(f"Generated {guids_generated} guids in {self.csv_file.file_location}")

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
            self.data_set_has_changed = True

        logging.info(f"Set {self.csv_file.file_location} data; changed {changed}, "
                     f"added {added}, while {unchanged} were identical")

    def write_file_on_close(self):
        if self.data_set_has_changed:
            self.write_to_csv(list(self.compiled_data.values()))
