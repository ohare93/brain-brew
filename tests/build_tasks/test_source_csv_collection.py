import pytest
from unittest.mock import MagicMock, patch

from brain_brew.build_tasks.build_task_generic import BuildConfigKeys
from brain_brew.build_tasks.source_csv import SourceCsv
from brain_brew.build_tasks.source_csv_collection import SourceCsvCollection
from brain_brew.constants.deckpart_keys import DeckPartNoteKeys
from brain_brew.representation.json.deck_part_notes import DeckPartNotes
from tests.test_helpers import global_config
from tests.test_files import TestFiles
from tests.representation.generic.test_csv_file import csv_test1, csv_test2, csv_test1_split1, csv_test1_split2
from tests.representation.json.test_deck_part_notes import dp_notes_test1, dp_notes_test2
from tests.build_tasks.test_source_csv import csv_source_test1, csv_source_test2, csv_source_default,\
    csv_source_default2, csv_source_test1_split1, csv_source_test1_split2
from tests.representation.json.test_deck_part_note_model import dp_note_model_test1


@pytest.fixture()
def csv_collection_default() -> SourceCsvCollection:
    return SourceCsvCollection(
        {
            BuildConfigKeys.NOTES.value: "",
            BuildConfigKeys.SUBCONFIG.value: []
        },
        read_now=False
    )


@pytest.fixture()
def csv_collection_test1(csv_collection_default, dp_notes_test1, csv_source_test1) -> SourceCsvCollection:
    csv_collection_default.notes = dp_notes_test1
    csv_collection_default.source_csvs = [csv_source_test1]

    return csv_collection_default


@pytest.fixture()
def csv_collection_test2(csv_collection_default, dp_notes_test1, csv_source_test2) -> SourceCsvCollection:
    csv_collection_default.notes = dp_notes_test1
    csv_collection_default.source_csvs = [csv_source_test2]

    return csv_collection_default


@pytest.fixture()
def csv_collection_test_join_splits(csv_collection_default, dp_notes_test1,
                                    csv_source_test1_split1, csv_source_test1_split2) -> SourceCsvCollection:
    csv_collection_default.notes = dp_notes_test1
    csv_collection_default.source_csvs = [csv_source_test1_split1, csv_source_test1_split2]

    return csv_collection_default


class TestConstructor:
    def test_runs(self, csv_collection_test1):
        assert isinstance(csv_collection_test1, SourceCsvCollection)

    def test_pass_nothing(self):
        source = SourceCsvCollection(
            {
                BuildConfigKeys.NOTES.value: "",
                BuildConfigKeys.SUBCONFIG.value: []
            },
            read_now=False
        )

        assert isinstance(source, SourceCsvCollection)
        assert len(source.source_csvs) == 0

    # def test_passes_data_to_source_csvs_correctly(self):
    #     def setup_config(x, y):
    #         x.config_entry = y
    #
    #     with patch.object(SourceCsvCollection, "setup_config_with_subconfig_replacement", setup_config), \
    #          patch.object(SourceCsvCollection, "verify_config_entry", lambda x: None):
    #         return SourceCsvCollection(
    #             {
    #                 BuildConfigKeys.NOTES.value: "",
    #                 BuildConfigKeys.SUBCONFIG.value: [
    #                     csv_source_default.config_entry
    #                 ]
    #             },
    #             read_now=False
    #         )


class TestSourceToDeckParts:
    def test_runs_first(self, csv_collection_test1: SourceCsvCollection, dp_notes_test1: DeckPartNotes):
        def assert_format(notes_data):
            assert notes_data == dp_notes_test1.get_data()[DeckPartNoteKeys.NOTES.value]

        with patch.object(DeckPartNotes, "set_data", side_effect=assert_format) as mock_set_data:
            csv_collection_test1.source_to_deck_parts()

            assert mock_set_data.call_count == 1

    def test_runs_second(self, csv_collection_test2: SourceCsvCollection, dp_notes_test2: DeckPartNotes):
        def assert_format(notes_data):
            assert notes_data == dp_notes_test2.get_data()[DeckPartNoteKeys.NOTES.value]

        with patch.object(DeckPartNotes, "set_data", side_effect=assert_format) as mock_set_data:
            csv_collection_test2.source_to_deck_parts()

            assert mock_set_data.call_count == 1

    def test_combines_multiple_csv_mappings(self, csv_collection_test_join_splits: SourceCsvCollection,
                                            dp_notes_test1: DeckPartNotes):
        def assert_format(notes_data):
            assert notes_data == dp_notes_test1.get_data()[DeckPartNoteKeys.NOTES.value]

        with patch.object(DeckPartNotes, "set_data", side_effect=assert_format) as mock_set_data:
            csv_collection_test_join_splits.source_to_deck_parts()

            assert mock_set_data.call_count == 1


class TestDeckPartsToSource:
    pass
