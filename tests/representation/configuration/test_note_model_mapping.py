from typing import Dict, List
from unittest.mock import patch

import pytest

from brain_brew.representation.configuration.note_model_mapping import NoteModelMapping, FieldMapping, \
    NOTE_MODEL, COLUMNS, PERSONAL_FIELDS
from brain_brew.representation.generic.csv_file import CsvFile
from brain_brew.representation.json.deck_part_notemodel import DeckPartNoteModel
from tests.representation.configuration.test_global_config import global_config
from tests.test_file_manager import get_new_file_manager
from tests.representation.generic.test_csv_file import csv_test1


def setup_nmm_config(note_model: str, field_mappings: Dict[str, str], personal_fields: List[str]):
    return {
        NOTE_MODEL: note_model,
        COLUMNS: field_mappings,
        PERSONAL_FIELDS: personal_fields
    }


@pytest.fixture()
def nmm_test1(global_config) -> NoteModelMapping:
    get_new_file_manager()
    config = setup_nmm_config(
        "Test Model",
        {
            "guid": "guid",
            "tags": "tags",

            "english": "word",
            "danish": "otherword"
        },
        []
    )
    return NoteModelMapping(config, True)


@pytest.fixture()
def nmm_test_with_personal_fields1(global_config) -> NoteModelMapping:
    get_new_file_manager()
    config = setup_nmm_config(
        "Test Model",
        {
            "guid": "guid",
            "tags": "tags",

            "english": "word",
            "danish": "otherword"
        },
        ["extra", "morph_focus"]
    )
    return NoteModelMapping(config, True)


class TestInit:
    def test_runs(self):
        nmm = NoteModelMapping(setup_nmm_config("test", {}, []), read_now=False)
        assert isinstance(nmm, NoteModelMapping)

    @pytest.mark.parametrize("read_file_now, note_model, personal_fields, columns", [
        (False, "note_model.json", ["x"], {"guid": "guid", "tags": "tags", "english": "word", "danish": "otherword"}),
        (True, "model_model", [], {"guid": "guid", "tags": "tags"}),
        (False, "note_model-json", ["x", "y", "z"], {"guid": "guid", "tags": "tags", "english": "word", "danish": "otherword"})
    ])
    def test_values(self, read_file_now, note_model, personal_fields, columns):
        config = setup_nmm_config(note_model, columns, personal_fields)

        def assert_dp_note_model(passed_file, read_now):
            assert passed_file == note_model
            assert read_now == read_file_now

        with patch.object(DeckPartNoteModel, "create", side_effect=assert_dp_note_model) as mock_nm:

            nmm = NoteModelMapping(config, read_now=read_file_now)

            assert isinstance(nmm, NoteModelMapping)
            assert len(nmm.columns) == len(columns)
            assert len(nmm.personal_fields) == len(personal_fields)

            assert mock_nm.call_count == 1


class TestVerifyContents:
    pass  # TODO


class TestCsvRowNoteFieldConversion:
    @staticmethod
    def get_csv_row(): return {
        "guid": "AAAA",
        "tags": "nice card",

        "english": "what",
        "danish": "hvad"
    }

    @staticmethod
    def get_note_field(): return{
        "guid": "AAAA",
        "tags": "nice card",

        "word": "what",
        "otherword": "hvad",
        "extra": False,
        "morph_focus": False
    }

    def test_csv_row_map_to_note_fields(self, nmm_test_with_personal_fields1):
        assert nmm_test_with_personal_fields1.csv_row_map_to_note_fields(self.get_csv_row()) == self.get_note_field()

    def test_note_fields_map_to_csv_row(self, nmm_test_with_personal_fields1):
        assert nmm_test_with_personal_fields1.note_fields_map_to_csv_row(self.get_note_field()) == self.get_csv_row()


class TestGetRelevantData:
    def test_data_correct(self, nmm_test_with_personal_fields1: NoteModelMapping, csv_test1: CsvFile):
        expected_relevant_columns = ["guid", "english", "danish", "tags"]
        data = csv_test1.get_data()

        for row in data:
            relevant_data = nmm_test_with_personal_fields1.get_relevant_data(row)
            assert len(relevant_data) == 4
            assert list(relevant_data.keys()) == expected_relevant_columns

    def test_data_missing_columns(self, nmm_test_with_personal_fields1: NoteModelMapping, csv_test1: CsvFile):
        row_missing = {
            "guid": "test",
            "english": "test"
        }
        with pytest.raises(Exception) as e:
            relevant_data = nmm_test_with_personal_fields1.get_relevant_data(row_missing)

        errors = e.value.args[0]
        assert len(errors) == 2
        assert isinstance(errors[0], KeyError)
        assert isinstance(errors[1], KeyError)
        assert errors[0].args[0] == "Missing column tags"
        assert errors[1].args[0] == "Missing column danish"


class TestFieldMapping:
    def test_init(self):
        fm = FieldMapping(FieldMapping.FieldMappingType.COLUMN, "Csv_Row", "note_model_field")
        assert isinstance(fm, FieldMapping)
        assert (fm.field_name, fm.value) == ("csv_row", "note_model_field")
