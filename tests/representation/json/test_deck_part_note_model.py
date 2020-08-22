from unittest.mock import Mock

import pytest

from brain_brew.representation.yaml.note_model_repr import DeckPartNoteModel, CANoteModelKeys
from tests.test_files import TestFiles


def mock_dp_nm(name, read_now):
    mock = Mock()
    mock.name = name
    return mock


class TestConstructor:
    @pytest.mark.parametrize("note_model_name", [
        TestFiles.CrowdAnkiNoteModels.TEST_COMPLETE,
        TestFiles.CrowdAnkiNoteModels.TEST,
    ])
    def test_run(self, global_config, note_model_name):
        file = DeckPartNoteModel(note_model_name)

        assert isinstance(file, DeckPartNoteModel)
        assert file.file_location == TestFiles.CrowdAnkiNoteModels.LOC + TestFiles.CrowdAnkiNoteModels.TEST_COMPLETE
        assert len(file.get_data().keys()) == 13

        assert file.name == "Test Model"
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
def dp_note_model_test1(global_config) -> DeckPartNoteModel:
    return DeckPartNoteModel.create_or_get(TestFiles.CrowdAnkiNoteModels.TEST_COMPLETE)


def test_read_fields(dp_note_model_test1):
    expected = ["Word", "OtherWord"]
    assert dp_note_model_test1.fields == expected


@pytest.mark.parametrize("fields", ([
    ["word", "unknown field"],
    ["Word", "Unknown Field"]
]))
def test_check_field_overlap(dp_note_model_test1, fields):
    expected_extra = ["unknown field"]
    expected_missing = ["otherword"]

    missing, extra = dp_note_model_test1.check_field_overlap(fields)

    assert missing == expected_missing
    assert extra == expected_extra


class TestZipFieldToData:
    def test_runs_normally(self, dp_note_model_test1):
        data_to_zip = ["Example word", "Another one"]
        expected = {"word": "Example word", "otherword": "Another one"}

        zipped = dp_note_model_test1.zip_field_to_data(data_to_zip)

        assert zipped == expected

    def test_raises_exception_when_list_length_differs(self, dp_note_model_test1):
        with pytest.raises(Exception):
            dp_note_model_test1.zip_field_to_data(["Example"])


@pytest.fixture()
def temp_dp_note_model_file(tmpdir) -> DeckPartNoteModel:
    file = tmpdir.mkdir("note_models").join("file.json")
    file.write("{}")

    return DeckPartNoteModel(file.strpath, read_now=False)
