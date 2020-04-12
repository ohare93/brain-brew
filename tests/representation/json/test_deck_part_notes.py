import pytest

from brain_brew.representation.json.json_file import JsonFile
from brain_brew.representation.json.deck_part_notes import DeckPartNotes
from tests.test_files import TestFiles
from tests.test_helpers import global_config, note_models_mock


@pytest.fixture()
def dp_note_test1(global_config) -> DeckPartNotes:
    return DeckPartNotes(TestFiles.NoteFiles.NO_GROUPING_OR_SHARED_TAGS)


class TestConstructor:
    @pytest.mark.parametrize("file_to_read", [
        TestFiles.NoteFiles.WITH_SHARED_TAGS_EMPTY_AND_GROUPING,
        TestFiles.NoteFiles.WITH_SHARED_TAGS,
        TestFiles.NoteFiles.WITH_GROUPING,
        TestFiles.NoteFiles.NO_GROUPING_OR_SHARED_TAGS,
    ])
    def test_runs_and_unstructures_data(self, file_to_read, global_config, dp_note_test1):
        expected_result = dp_note_test1.get_data()
        notes = DeckPartNotes(file_to_read)

        assert isinstance(notes, DeckPartNotes)
        assert notes.get_data() == expected_result


@pytest.fixture()
def dp_notes_test1(global_config) -> DeckPartNotes:
    return DeckPartNotes.create(TestFiles.NoteFiles.WITH_SHARED_TAGS_EMPTY_AND_GROUPING)


@pytest.fixture()
def temp_dp_notes_file(tmpdir) -> DeckPartNotes:
    file = tmpdir.mkdir("notes").join("file.json")
    file.write("{}")

    return DeckPartNotes(file.strpath, read_now=False)

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
