import pytest

from brain_brew.representation.json.json_file import JsonFile
from brain_brew.representation.json.deck_part_notes import DeckPartNotes, DeckPartNoteKeys
from tests.test_files import TestFiles
from tests.test_helpers import global_config, note_models_mock


class TestConstructor:
    @pytest.mark.parametrize("file_to_read", [
        TestFiles.NoteFiles.TEST1_WITH_SHARED_TAGS_EMPTY_AND_GROUPING,
        TestFiles.NoteFiles.TEST1_WITH_SHARED_TAGS,
        TestFiles.NoteFiles.TEST1_WITH_GROUPING,
        TestFiles.NoteFiles.TEST1_NO_GROUPING_OR_SHARED_TAGS,
    ])
    def test_runs_and_unstructures_data(self, file_to_read, global_config, dp_notes_test1):
        expected_result = dp_notes_test1.get_data()
        notes = DeckPartNotes(file_to_read)

        assert isinstance(notes, DeckPartNotes)
        assert notes.get_data() == expected_result


@pytest.fixture()
def dp_notes_test1(global_config) -> DeckPartNotes:
    return DeckPartNotes.create(TestFiles.NoteFiles.TEST1_WITH_SHARED_TAGS_EMPTY_AND_GROUPING)


@pytest.fixture()
def dp_notes_test2(global_config) -> DeckPartNotes:
    return DeckPartNotes.create(TestFiles.NoteFiles.TEST2_WITH_SHARED_TAGS_AND_GROUPING)


@pytest.fixture()
def temp_dp_notes_file(tmpdir) -> DeckPartNotes:
    file = tmpdir.mkdir("notes").join("file.json")
    file.write("{}")

    return DeckPartNotes(file.strpath, read_now=False)


class TestSortData:
    @pytest.mark.parametrize("keys, reverse, result_column, expected_results", [
        (["guid"], False, "guid", [(0, "AAAA"), (1, "BBBB"), (2, "CCCC"), (14, "OOOO")]),
        (["guid"], True, "guid", [(14, "AAAA"), (13, "BBBB"), (12, "CCCC"), (0, "OOOO")]),
        # (["word"], False, "word", [(0, "banana"), (1, "bird"), (2, "cat"), (14, "you")]),
        # (["word"], True, "word", [(14, "banana"), (13, "bird"), (12, "cat"), (0, "you")]),
        # (["tags"], False, "tags", [(0, "besttag"), (1, "funny"), (2, "tag2, tag3"), (13, ""), (14, "")]),
    ])
    def test_sort(self, dp_notes_test1: DeckPartNotes, keys, reverse, result_column, expected_results):
        dp_notes_test1.sort_data(
            keys, reverse, False
        )

        sorted_data = dp_notes_test1.get_data()[DeckPartNoteKeys.NOTES.value]

        for result in expected_results:
            assert sorted_data[result[0]][result_column] == result[1]

    def test_insensitive(self):
        pass


# def test_set_data_from_override():
#     assert False
#
#
# def test_write_file():
#     assert False
#
#
# def test_read_file():
#     assert False
#
#
# def test_interpret_data():
#     assert False
#
#
# def test_read_note_config():
#     assert False
#
#
# def test_implement_note_structure():
#     assert False
#
#
# def test_remove_notes_structure():
#     assert False
