from unittest.mock import patch

import pytest

from brain_brew.constants.build_config_keys import BuildConfigKeys
from brain_brew.build_tasks.source_crowd_anki import SourceCrowdAnki, CrowdAnkiKeys
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.representation.json.deck_part_header import DeckPartHeader
from brain_brew.representation.json.deck_part_notemodel import DeckPartNoteModel
from brain_brew.representation.json.deck_part_notes import DeckPartNotes
from tests.test_files import TestFiles
from tests.representation.json.test_crowd_anki_export import ca_export_test1, temp_ca_export_file
from tests.representation.json.test_deck_part_header import dp_headers_test1, temp_dp_headers_file
from tests.representation.json.test_deck_part_notes import dp_notes_test1, temp_dp_notes_file
from tests.representation.json.test_deck_part_note_model import dp_note_model_test1, temp_dp_note_model_file
from tests.representation.json.test_json_file import temp_json_file
from tests.representation.configuration.test_global_config import global_config


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

        with patch.object(DeckPartHeader, "create", side_effect=assert_dp_header) as mock_header, \
                patch.object(DeckPartNotes, "create", side_effect=assert_dp_notes) as mock_notes, \
                patch.object(CrowdAnkiExport, "create", side_effect=assert_ca_export) as ca_export:

            source = SourceCrowdAnki(config, read_now=read_file_now)

            assert isinstance(source, SourceCrowdAnki)
            assert source.should_handle_media == media
            assert source.useless_note_keys == useless_note_keys

            assert mock_header.call_count == 1
            assert mock_notes.call_count == 1
            assert ca_export.call_count == 1


@pytest.fixture()
def source_crowd_anki_test1(global_config) -> SourceCrowdAnki:
    with patch.object(DeckPartHeader, "create", return_value=None) as mock_header, \
         patch.object(DeckPartNotes, "create", return_value=None) as mock_notes, \
         patch.object(CrowdAnkiExport, "create", return_value=None) as mock_ca_export:

        source = SourceCrowdAnki(
            setup_ca_config("", False, {"__type__": "Note", "data": None, "flags": 0}, "", "")
        )

        # source.notes = dp_notes_test1
        # source.headers = dp_headers_test1
        # source.crowd_anki_export = ca_export_test1

    return source


class TestSourceToDeckParts:
    def test_runs(self, source_crowd_anki_test1: SourceCrowdAnki, ca_export_test1,
                  temp_dp_note_model_file, temp_dp_headers_file, temp_dp_notes_file,
                  dp_note_model_test1, dp_headers_test1, dp_notes_test1):

        # CrowdAnki Export it will use to write to the DeckParts
        source_crowd_anki_test1.crowd_anki_export = ca_export_test1

        # DeckParts to be written to (+ the NoteModel below)
        source_crowd_anki_test1.headers = temp_dp_headers_file
        source_crowd_anki_test1.notes = temp_dp_notes_file

        def assert_note_model(name, data_override):
            assert data_override == dp_note_model_test1.get_data()
            return dp_note_model_test1

        with patch.object(DeckPartNoteModel, "create", side_effect=assert_note_model) as mock_nm:
            source_crowd_anki_test1.source_to_deck_parts()

            assert source_crowd_anki_test1.headers.get_data() == dp_headers_test1.get_data()
            assert source_crowd_anki_test1.notes.get_data() == dp_notes_test1.get_data()

            assert mock_nm.call_count == 1


class TestDeckPartsToSource:
    def test_runs(self, source_crowd_anki_test1: SourceCrowdAnki, temp_ca_export_file,
                  ca_export_test1, dp_notes_test1, dp_headers_test1):
        source_crowd_anki_test1.crowd_anki_export = temp_ca_export_file  # File to write result to

        # DeckParts it will use (+ dp_note_model_test1, but it reads that in as a file)
        source_crowd_anki_test1.headers = dp_headers_test1
        source_crowd_anki_test1.notes = dp_notes_test1

        source_crowd_anki_test1.deck_parts_to_source()  # Where the magic happens

        assert temp_ca_export_file.get_data() == ca_export_test1.get_data()
