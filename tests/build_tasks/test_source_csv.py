from typing import List
from unittest.mock import patch

import pytest

from brain_brew.build_tasks.source_csv import SourceCsv, SourceCsvKeys
from brain_brew.constants.deckpart_keys import DeckPartNoteKeys
from brain_brew.representation.configuration.csv_file_mapping import FileMapping
from brain_brew.representation.configuration.note_model_mapping import NoteModelMapping
from brain_brew.representation.generic.csv_file import CsvFile
from brain_brew.representation.generic.generic_file import SourceFile
from brain_brew.representation.json.deck_part_notes import DeckPartNotes
from tests.representation.json.test_deck_part_notes import dp_notes_test1
from tests.representation.configuration.test_note_model_mapping import setup_nmm_config
from tests.representation.configuration.test_csv_file_mapping import setup_csv_fm_config


def setup_source_csv_config(notes: str, nmm: list, csv_mappings: list):
    return {
        SourceCsvKeys.NOTES.value: notes,
        SourceCsvKeys.NOTE_MODEL_MAPPINGS.value: nmm,
        SourceCsvKeys.CSV_MAPPINGS.value: csv_mappings
    }


def get_csv_default(notes: DeckPartNotes, nmm: List[NoteModelMapping], csv_maps: List[FileMapping]) -> SourceCsv:
    csv_source = SourceCsv(setup_source_csv_config("", [], []), read_now=False)

    csv_source.notes = notes
    csv_source.note_model_mappings_dict = {nm_map.note_model.name: nm_map for nm_map in nmm}
    csv_source.csv_file_mappings = csv_maps

    return csv_source


@pytest.fixture()
def csv_source_test1(dp_notes_test1, nmm_test1, csv_file_mapping1) -> SourceCsv:
    return get_csv_default(dp_notes_test1, [nmm_test1], [csv_file_mapping1])


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
def csv_source_test2(dp_notes_test2, nmm_test1, csv_file_mapping2) -> SourceCsv:
    return get_csv_default(dp_notes_test2, [nmm_test1], [csv_file_mapping2])


# @pytest.fixture()
# def temp_csv_source(global_config, tmpdir) -> SourceCsv:
#     file = tmpdir.mkdir("notes").join("file.csv")
#     file.write("test,1,2,3")


class TestConstructor:
    def test_runs(self):
        source_csv = get_csv_default(None, [], [])
        assert isinstance(source_csv, SourceCsv)

    @pytest.mark.parametrize("notes, model, columns, personal_fields, csv_file", [
        ("notes.json", "Test Model", {"a": "b"}, ["extra"], "file.csv")
    ])
    def test_calls_correctly(self, notes, model, columns, personal_fields, csv_file, nmm_test1):
        nmm_config = [setup_nmm_config(model, columns, personal_fields)]
        csv_config = [setup_csv_fm_config(csv_file, note_model_name=model)]

        def assert_csv(config, read_now):
            assert config in csv_config
            assert read_now is False

        def assert_nmm(config, read_now):
            assert config in nmm_config
            assert read_now is False

        def assert_dpn(config, read_now):
            assert config == notes
            assert read_now is False

        with patch.object(FileMapping, "__init__", side_effect=assert_csv), \
                patch.object(NoteModelMapping, "__init__", side_effect=assert_nmm), \
                patch.object(NoteModelMapping, "note_model"), \
                patch.object(DeckPartNotes, "create", side_effect=assert_dpn):

            #nmm_mock.return_value = False

            source_csv = SourceCsv(setup_source_csv_config(
                notes,
                nmm_config,
                csv_config
            ), read_now=False)


    # def test_missing_non_required_columns


class TestSourceToDeckParts:
    def test_runs_first(self, csv_source_test1, dp_notes_test1, csv_source_test2, dp_notes_test2):
        self.run_s2dp(csv_source_test1, dp_notes_test1)
        self.run_s2dp(csv_source_test2, dp_notes_test2)

    @staticmethod
    def run_s2dp(csv_source: SourceCsv, dp_notes: DeckPartNotes):
        def assert_format(notes_data):
            assert notes_data == dp_notes.get_data()[DeckPartNoteKeys.NOTES.value]

        with patch.object(DeckPartNotes, "set_data", side_effect=assert_format) as mock_set_data:
            csv_source.source_to_deck_parts()
            assert mock_set_data.call_count == 1


class TestDeckPartsToSource:
    def test_runs_with_no_change(self, csv_source_test1, csv_test1, csv_source_test2, csv_test2):

        self.run_dpts(csv_source_test1, csv_test1)
        self.run_dpts(csv_source_test2, csv_test2)

    @staticmethod
    def run_dpts(csv_source: SourceCsv, csv_file: CsvFile):
        def assert_format(source_data):
            assert source_data == csv_file.get_data()

        with patch.object(SourceFile, "set_data", side_effect=assert_format) as mock_set_data:
            csv_source.deck_parts_to_source()
            assert csv_source.csv_file_mappings[0].data_set_has_changed is False

            csv_source.csv_file_mappings[0].data_set_has_changed = True
            csv_source.csv_file_mappings[0].write_file_on_close()
            assert mock_set_data.call_count == 1

