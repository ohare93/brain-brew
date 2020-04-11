from unittest.mock import patch

from brain_brew.build_tasks.build_task_generic import BuildConfigKeys
from brain_brew.build_tasks.source_crowd_anki import SourceCrowdAnki, CrowdAnkiKeys
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from tests.test_files import TestFiles
from tests.test_helpers import *


def setup_ca_config(file, media, useless_note_keys, notes, headers):
    return {
        CrowdAnkiKeys.FILE.value: file,
        CrowdAnkiKeys.MEDIA.value: media,
        CrowdAnkiKeys.USELESS_NOTE_KEYS.value: useless_note_keys,
        BuildConfigKeys.NOTES.value: notes,
        BuildConfigKeys.HEADERS.value: headers
    }


class TestConstructor:
    @pytest.mark.parametrize("file, media, useless_note_keys, notes, headers, read_file_now", [
        ("test", False, {}, "test.json", "header.json", False),
        ("export1", True, {}, "test.json", "header.json", False),
        ("json.json", False, {}, "test.json", "", True),
        ("", False, {"__type__": "Note", "data": None, "flags": 0}, "test.json", "header.json", False)
    ])
    def test_runs(self, file, media, useless_note_keys, notes, headers, read_file_now, global_config):
        config = setup_ca_config(file, media, useless_note_keys, notes, headers)

        def assert_dp_header(passed_file, read_now):
            assert passed_file == headers
            assert read_now == read_file_now

        def assert_dp_notes(passed_file, read_now):
            assert passed_file == notes
            assert read_now == read_file_now

        def assert_ca_export(passed_file, read_now):
            assert passed_file == file
            assert read_now == read_file_now

        with patch.object(DeckPartHeader, "create", assert_dp_header), \
             patch.object(DeckPartNotes, "create", assert_dp_notes), \
             patch.object(CrowdAnkiExport, "create", assert_ca_export):
            source = SourceCrowdAnki(config, read_now=read_file_now)

            assert isinstance(source, SourceCrowdAnki)
            assert source.should_handle_media == media
            assert source.useless_note_keys == useless_note_keys


@pytest.fixture()
def source_crowd_anki_test1(ca_export_test1, dp_headers_test1, dp_notes_test1, global_config_with_mock_dp_folder):
    with patch.object(DeckPartHeader, "create", lambda x, y: None), \
         patch.object(DeckPartNotes, "create", lambda x, y: None), \
         patch.object(CrowdAnkiExport, "create", lambda x, y: None):

        source = SourceCrowdAnki(
            setup_ca_config("", False, {"__type__": "Note", "data": None, "flags": 0}, "", "")
        )

        source.notes = dp_notes_test1
        source.headers = dp_headers_test1
        source.crowd_anki_export = ca_export_test1

    return source


# class TestSourceToDeckParts:
#     def test_runs(self, source_crowd_anki_test1, dp_note_model_ll_noun, dp_headers_test1, dp_notes_test1):
#         source_crowd_anki_test1.source_to_deck_parts()
#
#         assert headers == headers_mock.get_data()
#         assert len(note_models) == len(note_models_mock)
#
#
#
#         assert notes == notes_mock.get_data()
#
#
# class TestDeckPartsToSource:
#     def test_runs(self):
#         global_config = get_global_config()
#         temp_dir, temp_file = setup_temp_file_in_folder(".json")
#
#         source = setup_source(temp_dir.name)
#         source_json = source.deck_parts_to_source()
#
#         target_json = JsonFile(TestFiles.CrowdAnkiExport.TEST1_JSON, read_now=True)
#
#         # debug_write_to_target_json(source_json, TestFiles.CrowdAnkiJson.TEST1_JSON)
#         assert source_json == target_json.get_data()
