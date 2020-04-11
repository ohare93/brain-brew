import pytest

from brain_brew.representation.json.json_file import JsonFile
from brain_brew.representation.json.deck_part_notes import DeckPartNotes
from tests.test_files import TestFiles
from tests.test_helpers import global_config, note_models_mock


class TestConstructor:
    @pytest.mark.parametrize("file_to_read", [
        TestFiles.NoteFiles.WITH_SHARED_TAGS_EMPTY_AND_GROUPING,
        TestFiles.NoteFiles.WITH_SHARED_TAGS,
        TestFiles.NoteFiles.WITH_GROUPING,
        TestFiles.NoteFiles.NO_GROUPING_OR_SHARED_TAGS,
    ])
    def test_runs_and_unstructures_data(self, file_to_read, global_config, note_models_mock):
        expected_result = JsonFile(TestFiles.NoteFiles.NO_GROUPING_OR_SHARED_TAGS).get_data()
        notes = DeckPartNotes(file_to_read)

        assert isinstance(notes, DeckPartNotes)
        assert notes.get_data() == expected_result


@pytest.fixture()
def dp_notes_test1():
    return DeckPartNotes.create(TestFiles.NoteFiles.WITH_SHARED_TAGS_EMPTY_AND_GROUPING)

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
