from unittest.mock import patch

import pytest

from brain_brew.build_tasks.source_csv import SourceCsv
from brain_brew.constants.deckpart_keys import DeckPartNoteKeys
from brain_brew.representation.generic.csv_file import CsvFile
from brain_brew.representation.json.deck_part_notemodel import DeckPartNoteModel
from brain_brew.representation.json.deck_part_notes import DeckPartNotes
from tests.test_files import TestFiles
from tests.test_helpers import global_config
from tests.representation.generic.test_csv_file import csv_test1, csv_test2, csv_test1_split1, csv_test1_split2
from tests.representation.json.test_deck_part_notes import dp_notes_test1, dp_notes_test2
from tests.representation.json.test_deck_part_note_model import dp_note_model_test1


def setup_csv_source_config(notes: str, note_model: str, csv: str, sort_by_columns: list,
                            reverse_sort: bool, personal_fields: list, columns: dict):
    return {
        "notes": notes,
        "csv": csv,
        "note_model": note_model,
        "sort_by_columns": sort_by_columns,
        "reverse_sort": reverse_sort,
        "columns": columns,
        "personal_fields": personal_fields
    }


def get_csv_default(dp_note_model_test1) -> SourceCsv:
    config = setup_csv_source_config("", "", "", [], False, [],
                                     {"guid": "guid", "tags": "tags", "english": "word", "danish": "otherword"})

    csv_source = SourceCsv(config, read_now=False)
    csv_source.note_model = dp_note_model_test1
    return csv_source


@pytest.fixture(name="csv_source_default")
def csv_source_default(dp_note_model_test1) -> SourceCsv:
    return get_csv_default(dp_note_model_test1)


@pytest.fixture(name="csv_source_default2")
def csv_source_default2(dp_note_model_test1) -> SourceCsv:
    return get_csv_default(dp_note_model_test1)


@pytest.fixture()
def csv_source_test1(csv_source_default, csv_test1, dp_notes_test1) -> SourceCsv:
    csv_source_default.csv_file = csv_test1
    csv_source_default.notes = dp_notes_test1
    return csv_source_default


@pytest.fixture()
def csv_source_test1_split1(csv_source_default, csv_test1_split1, dp_notes_test1) -> SourceCsv:
    csv_source_default.csv_file = csv_test1_split1
    csv_source_default.notes = dp_notes_test1
    return csv_source_default


@pytest.fixture()
def csv_source_test1_split2(csv_source_default2, csv_test1_split2, dp_notes_test2) -> SourceCsv:
    csv_source_default2.csv_file = csv_test1_split2
    csv_source_default2.notes = dp_notes_test1
    return csv_source_default2


@pytest.fixture()
def csv_source_test2(csv_source_default, csv_test2, dp_notes_test2) -> SourceCsv:
    csv_source_default.csv_file = csv_test2
    csv_source_default.notes = dp_notes_test2
    return csv_source_default


# @pytest.fixture()
# def temp_csv_source(global_config, tmpdir) -> SourceCsv:
#     file = tmpdir.mkdir("notes").join("file.csv")
#     file.write("test,1,2,3")


class TestConstructor:
    @pytest.mark.parametrize("read_file_now, notes, note_model, csv, sort_by_columns, reverse_sort, personal_fields, columns", [
        (False, "notes.json", "note_model.json", "first.csv", ["guid"], False, ["x"],
            {"guid": "guid", "tags": "tags", "english": "word", "danish": "otherword"}),
        (True, "othernotes.json", "model_model.json", "second.csv", ["guid", "note_model_name"], True, [],
         {"guid": "guid", "tags": "tags"}),
        (False, "notes.json", "note_model-json", "first.csv", ["guid"], False, ["x"],
             {"guid": "guid", "tags": "tags", "english": "word", "danish": "otherword"})
    ])
    def test_runs(self, read_file_now, notes, note_model, csv, sort_by_columns, reverse_sort, personal_fields, columns):
        config = setup_csv_source_config(notes, note_model, csv, sort_by_columns,
                                         reverse_sort, personal_fields, columns)

        def assert_dp_note_model(passed_file, read_now):
            assert passed_file == note_model
            assert read_now == read_file_now

        def assert_dp_notes(passed_file, read_now):
            assert passed_file == notes
            assert read_now == read_file_now

        def assert_csv(passed_file, read_now):
            assert passed_file == csv
            assert read_now == read_file_now

        with patch.object(DeckPartNoteModel, "create", side_effect=assert_dp_note_model) as mock_nm, \
             patch.object(DeckPartNotes, "create", side_effect=assert_dp_notes) as mock_notes, \
             patch.object(CsvFile, "create", side_effect=assert_csv) as mock_csv:

            source_csv = SourceCsv(config, read_now=read_file_now)

            assert isinstance(source_csv, SourceCsv)
            assert len(source_csv.columns) == len(columns)
            assert len(source_csv.personal_fields) == len(personal_fields)
            assert source_csv.reverse_sort == reverse_sort
            assert source_csv.sort_by_columns == sort_by_columns

            assert mock_nm.call_count == 1
            assert mock_notes.call_count == 1
            assert mock_csv.call_count == 1

    # def test_missing_non_required_columns


class TestSourceToDeckParts:
    def test_runs_first(self, csv_source_test1: SourceCsv, dp_notes_test1: DeckPartNotes):
        def assert_format(notes_data):
            assert notes_data == dp_notes_test1.get_data()[DeckPartNoteKeys.NOTES.value]

        with patch.object(DeckPartNotes, "set_data", side_effect=assert_format) as mock_set_data:
            csv_source_test1.source_to_deck_parts()
            assert mock_set_data.call_count == 1

    def test_runs_second(self, csv_source_test2: SourceCsv, dp_notes_test2: DeckPartNotes):
        def assert_format(notes_data):
            assert notes_data == dp_notes_test2.get_data()[DeckPartNoteKeys.NOTES.value]

        with patch.object(DeckPartNotes, "set_data", side_effect=assert_format) as mock_set_data:
            csv_source_test2.source_to_deck_parts()
            assert mock_set_data.call_count == 1


class TestDeckPartsToSource:
    relevant_columns = ["english", "danish", "guid", "tags"]

    def test_runs_first(self, csv_source_test1: SourceCsv, csv_test1: CsvFile):
        def assert_format(source_data):
            assert source_data == csv_test1.get_relevant_data(self.relevant_columns)

        with patch.object(CsvFile, "set_relevant_data", side_effect=assert_format) as mock_set_data:
            csv_source_test1.deck_parts_to_source()
            assert mock_set_data.call_count == 1

    def test_runs_second(self, csv_source_test2: SourceCsv, csv_test2: CsvFile):
        def assert_format(source_data):
            assert source_data == csv_test2.get_relevant_data(self.relevant_columns)

        with patch.object(CsvFile, "set_relevant_data", side_effect=assert_format) as mock_set_data:
            csv_source_test2.deck_parts_to_source()
            assert mock_set_data.call_count == 1


# TODO: Tests still remaining
#  - Sorting
#  - Incorrect values return appropriate errors
#  - The rest of the functions in SourceCsv, not just the data transformers
