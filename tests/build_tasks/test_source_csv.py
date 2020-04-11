import pytest

from brain_brew.build_tasks.source_csv import SourceCsv
from brain_brew.representation.generic.csv_file import CsvFile
from tests.test_files import TestFiles
from tests.test_helpers import *


def setup_source(file_name, read_now=False) -> SourceCsv:
    return SourceCsv(
        {
            "notes": "",
            "csv": file_name,
            "note_model": TestFiles.NoteModels.LL_NOUN_FULL,
            "columns": {
                "guid": "guid",
                "tags": "tags",
                "english": "word",
                "danish": "otherword"
            },
            "personal_fields": ["extra"]

        },
        read_now=read_now
    )

#
# class TestConstructor:
#     def test_runs(self):
#         global_config = get_global_config()
#         deck_parts = get_header_model_notes_mock(
#             deck_part_notes=JsonFile(TestFiles.UnfinishedData.FIRST_SET)
#         )
#         temp_dir, temp_file = setup_temp_file_in_folder(".csv")
#
#         source = setup_source(temp_dir.name, deck_parts)
#
#         assert isinstance(source, SourceCsv)
#
#
# class TestSourceToDeckParts:
#     @pytest.mark.parametrize("csv, unfinished_data", [
#         (TestFiles.CsvFiles.TEST1, TestFiles.UnfinishedData.FIRST_SET),
#         (TestFiles.CsvFiles.TEST2, TestFiles.UnfinishedData.SECOND_SET),
#         (TestFiles.CsvFiles.TEST1_SPLIT1, TestFiles.UnfinishedData.FIRST_SET_SPLIT1),
#         (TestFiles.CsvFiles.TEST1_SPLIT2, TestFiles.UnfinishedData.FIRST_SET_SPLIT2),
#     ])
#     def test_runs_fully(self, csv, unfinished_data):
#         global_config = get_global_config()
#         deck_parts = get_header_model_notes_mock()
#         source = setup_source(csv, read_now=True)
#         expected_result = JsonFile(unfinished_data).get_data()
#
#         notes = source.source_to_deck_parts()
#
#         # debug_write_to_target_json(notes, unfinished_data)
#
#         assert notes == expected_result
#
#
# class TestDeckPartsToSource:
#     @pytest.mark.parametrize("csv, notes_file", [
#         (TestFiles.CsvFiles.TEST1, TestFiles.NoteFiles.NO_GROUPING_OR_SHARED_TAGS),
#         (TestFiles.CsvFiles.TEST2, TestFiles.NoteFiles.WITH_SHARED_TAGS_AND_GROUPING)
#     ])
#     def test_runs_fully(self, csv, notes_file):
#         global_config = get_global_config()
#         deck_parts = get_header_model_notes_mock(notes_file_to_read=notes_file)
#
#         source = setup_source(csv, read_now=False)
#
#         expected_result = CsvFile(csv).get_relevant_data(["guid", "tags", "english", "danish"])
#
#         notes = source.deck_parts_to_source()
#
#         # debug_write_to_target_json(notes, unfinished_data)
#
#         assert notes == expected_result

# TODO: Tests still remaining
#  - Sorting
#  - Multiple Csvs
#  - Incorrect values return appropriate errors
#  - The rest of the functions in SourceCsv, not just the data transformers
