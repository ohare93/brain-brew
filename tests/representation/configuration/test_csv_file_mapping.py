import copy
from typing import List
from unittest.mock import patch

import pytest

from brain_brew.representation.configuration.csv_file_mapping import CsvFileMappingKeys, CsvFileMappingDerivative, \
    CsvFileMapping
from brain_brew.representation.generic.csv_file import CsvFile
from tests.test_file_manager import get_new_file_manager
from tests.representation.configuration.test_global_config import global_config
from tests.representation.generic.test_csv_file import csv_test1, csv_test2, csv_test3, csv_test1_split1, csv_test1_split2, csv_test2_missing_guids


def setup_csv_fm_config(csv: str, sort_by_columns: List[str] = None, reverse_sort: bool = None,
                        note_model_name: str = None, derivatives: List[dict] = None):
    cfm: dict = {
        CsvFileMappingKeys.CSV_FILE.value: csv
    }
    if sort_by_columns is not None:
        cfm.setdefault(CsvFileMappingKeys.SORT_BY_COLUMNS.value, sort_by_columns)
    if reverse_sort is not None:
        cfm.setdefault(CsvFileMappingKeys.REVERSE_SORT.value, reverse_sort)
    if note_model_name is not None:
        cfm.setdefault(CsvFileMappingKeys.NOTE_MODEL.value, note_model_name)
    if derivatives is not None:
        cfm.setdefault(CsvFileMappingKeys.DERIVATIVES.value, derivatives)

    return cfm


class TestConstructor:
    @pytest.mark.parametrize("read_file_now, note_model_name, csv, sort_by_columns, reverse_sort", [
        (False, "note_model.json", "first.csv", ["guid"], False),
        (True, "model_model.json", "second.csv", ["guid", "note_model_name"], True),
        (False, "note_model-json", "first.csv", ["guid"], False,)
    ])
    def test_runs_without_derivatives(self, global_config, read_file_now, note_model_name, csv,
                                      sort_by_columns, reverse_sort):
        get_new_file_manager()
        config = setup_csv_fm_config(csv, sort_by_columns, reverse_sort, note_model_name=note_model_name)

        def assert_csv(passed_file, read_now):
            assert passed_file == csv
            assert read_now == read_file_now

        with patch.object(CsvFileMappingDerivative, "create_derivative", return_value=None) as mock_derivatives, \
                patch.object(CsvFile, "create", side_effect=assert_csv) as mock_csv:

            csv_fm = CsvFileMapping(config, read_now=read_file_now)

            assert isinstance(csv_fm, CsvFileMapping)
            assert csv_fm.reverse_sort == reverse_sort
            assert csv_fm.sort_by_columns == sort_by_columns
            assert csv_fm.note_model_name == note_model_name

            assert mock_csv.call_count == 1
            assert mock_derivatives.call_count == 0

    @pytest.mark.parametrize("derivatives", [
        [setup_csv_fm_config("test_csv.csv")],
        [setup_csv_fm_config("test_csv.csv"), setup_csv_fm_config("second.csv")],
        [setup_csv_fm_config("a.csv"), setup_csv_fm_config("b.csv"), setup_csv_fm_config("c.csv")],
        [setup_csv_fm_config("a.csv", sort_by_columns=["word", "guid"], reverse_sort=True, note_model_name="d")],
        [setup_csv_fm_config("test_csv.csv", derivatives=[setup_csv_fm_config("der_der.csv")])],
    ])
    def test_runs_with_derivatives(self, global_config, derivatives: list):
        get_new_file_manager()
        config = setup_csv_fm_config("test", [], False, note_model_name="nm", derivatives=derivatives.copy())
        expected_call_count = len(derivatives)

        def assert_der(passed_file, read_now):
            der = derivatives.pop(0)
            assert passed_file == der
            assert read_now is False

        with patch.object(CsvFileMappingDerivative, "create_derivative", side_effect=assert_der) as mock_derivatives, \
                patch.object(CsvFile, "create", return_value=None):

            csv_fm = CsvFileMapping(config, read_now=False)

            assert mock_derivatives.call_count == len(csv_fm.derivatives) == expected_call_count


def csv_fixture_gen(csv_fix):
    with patch.object(CsvFile, "create", return_value=csv_fix):
        csv = CsvFileMapping(setup_csv_fm_config("", note_model_name="Test Model"))
        csv.compile_data()
        return csv


@pytest.fixture()
def csv_file_mapping1(csv_test1):
    return csv_fixture_gen(csv_test1)


@pytest.fixture()
def csv_file_mapping2(csv_test2):
    return csv_fixture_gen(csv_test2)


@pytest.fixture()
def csv_file_mapping3(csv_test3):
    return csv_fixture_gen(csv_test3)


@pytest.fixture()
def csv_file_mapping_split1(csv_test1_split1):
    return csv_fixture_gen(csv_test1_split1)


@pytest.fixture()
def csv_file_mapping_split1(csv_test1_split2):
    return csv_fixture_gen(csv_test1_split2)


@pytest.fixture()
def csv_file_mapping2_missing_guids(csv_test2_missing_guids):
    return csv_fixture_gen(csv_test2_missing_guids)


class TestSetRelevantData:
    def test_no_change(self, csv_file_mapping1: CsvFileMapping, csv_file_mapping_split1: CsvFileMapping):
        assert csv_file_mapping1.data_set_has_changed is False

        previous_data = csv_file_mapping1.compiled_data.copy()
        csv_file_mapping1.set_relevant_data(csv_file_mapping_split1.compiled_data)

        assert previous_data == csv_file_mapping1.compiled_data
        assert csv_file_mapping1.data_set_has_changed is False

    def test_change_but_no_extra(self, csv_file_mapping1: CsvFileMapping, csv_file_mapping2: CsvFileMapping):
        assert csv_file_mapping1.data_set_has_changed is False
        assert len(csv_file_mapping1.compiled_data) == 15

        previous_data = copy.deepcopy(csv_file_mapping1.compiled_data)
        csv_file_mapping1.set_relevant_data(csv_file_mapping2.compiled_data)

        assert previous_data != csv_file_mapping1.compiled_data
        assert csv_file_mapping1.data_set_has_changed is True
        assert len(csv_file_mapping1.compiled_data) == 15

    def test_change_extra_row(self, csv_file_mapping1: CsvFileMapping, csv_file_mapping3: CsvFileMapping):
        assert csv_file_mapping1.data_set_has_changed is False
        assert len(csv_file_mapping1.compiled_data) == 15

        previous_data = copy.deepcopy(csv_file_mapping1.compiled_data.copy())
        csv_file_mapping1.set_relevant_data(csv_file_mapping3.compiled_data)

        assert previous_data != csv_file_mapping1.compiled_data
        assert csv_file_mapping1.data_set_has_changed is True
        assert len(csv_file_mapping1.compiled_data) == 16


class TestCompileData:
    num = 0

    def get_num(self):
        self.num += 1
        return self.num

    def test_when_missing_guids(self, csv_file_mapping2_missing_guids: CsvFileMapping):
        with patch("brain_brew.representation.configuration.csv_file_mapping.generate_anki_guid", wraps=self.get_num) as mock_guid:

            csv_file_mapping2_missing_guids.compile_data()

            assert csv_file_mapping2_missing_guids.data_set_has_changed is True
            assert mock_guid.call_count == 9
            assert list(csv_file_mapping2_missing_guids.compiled_data.keys()) == list(range(1, 10))

#  Tests still to do:
#
#  Top level needs a NoteModel, others do not

