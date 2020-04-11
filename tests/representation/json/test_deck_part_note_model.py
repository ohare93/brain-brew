import pytest

from brain_brew.representation.json.deck_part_notemodel import DeckPartNoteModel
from tests.test_files import TestFiles
from tests.test_helpers import *


class TestConstructor:
    @pytest.mark.parametrize("note_model_name", [
        TestFiles.NoteModels.LL_NOUN_FULL,
        TestFiles.NoteModels.LL_NOUN,
    ])
    def test_run(self, global_config, note_model_name):
        file = DeckPartNoteModel(note_model_name)

        assert isinstance(file, DeckPartNoteModel)
        assert file.file_location == TestFiles.NoteModels.LL_NOUN_FULL
        assert len(file.get_data().keys()) == 13

        assert file.name == "LL Noun"
        assert file.id == "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"
        assert file.fields == ["Word", "OtherWord"]

    def test_config_location_override(self, global_config):
        loc = "place_for_stuff/"
        filename = "what-a-great-file.json"

        global_config.deck_parts.note_models = loc

        file = DeckPartNoteModel(filename, read_now=False, data_override={
            CANoteModelKeys.NAME.value: "name",
            CANoteModelKeys.ID.value: "id",
            CANoteModelKeys.FIELDS.value: []
        })

        assert file.file_location == loc + filename


@pytest.fixture()
def dp_note_model_ll_noun(global_config):
    return DeckPartNoteModel.create(TestFiles.NoteModels.LL_NOUN_FULL)


def test_read_fields(dp_note_model_ll_noun):
    expected = ["Word", "OtherWord"]
    assert dp_note_model_ll_noun.read_fields() == expected


@pytest.mark.parametrize("fields", ([
    ["word", "unknown field"],
    ["Word", "Unknown Field"]
]))
def test_check_field_overlap(dp_note_model_ll_noun, fields):
    expected_extra = ["unknown field"]
    expected_missing = ["otherword"]

    missing, extra = dp_note_model_ll_noun.check_field_overlap(fields)

    assert missing == expected_missing
    assert extra == expected_extra


class TestZipFieldToData:
    def test_runs_normally(self, dp_note_model_ll_noun):
        data_to_zip = ["Example word", "Another one"]
        expected = {"word": "Example word", "otherword": "Another one"}

        zipped = dp_note_model_ll_noun.zip_field_to_data(data_to_zip)

        assert zipped == expected

    def test_raises_exception_when_list_length_differs(self, dp_note_model_ll_noun):
        with pytest.raises(Exception):
            dp_note_model_ll_noun.zip_field_to_data(["Example"])
