import pytest
from unittest.mock import MagicMock

from brain_brew.build_tasks.build_task_generic import BuildConfigKeys
from brain_brew.build_tasks.source_csv_collection import SourceCsvCollection
from tests.test_files import TestFiles
from tests.test_helpers import *


def setup_source_csv_collection(unfinished_csv_data: list, read_now=False) -> SourceCsvCollection:
    csv_collection = SourceCsvCollection(
        {
            BuildConfigKeys.NOTES.value: "",
            BuildConfigKeys.SUBCONFIG.value: []
        },
        read_now=read_now
    )

    for csv_data in unfinished_csv_data:
        mapping_mock = MagicMock()
        mapping_mock.source_to_deck_parts.return_value = csv_data
        csv_collection.source_csvs.append(mapping_mock)

    return csv_collection


# class TestConstructor:
#     def test_pass_nothing(self):
#         global_config = get_global_config()
#
#         source = SourceCsvCollection(
#             {
#                 BuildConfigKeys.NOTES.value: "",
#                 BuildConfigKeys.SUBCONFIG.value: []
#             },
#             read_now=False
#         )
#
#         assert isinstance(source, SourceCsvCollection)
#         assert len(source.source_csvs) == 0
#
#     def test_config_entry_wrong_type(self):
#         global_config = get_global_config()
#         deck_parts = get_header_model_notes_mock()
#
#         with pytest.raises(TypeError):
#             SourceCsvCollection(
#                 1,
#                 read_now=False
#             )
#
#
# # def test_from_yaml():
# #     assert True
#
#
# class TestSourceToDeckParts:
#     @pytest.mark.parametrize("test_file, unfinished_data", [
#         (TestFiles.NoteFiles.NO_GROUPING_OR_SHARED_TAGS, TestFiles.UnfinishedData.FIRST_SET),
#         (TestFiles.NoteFiles.WITH_SHARED_TAGS_AND_GROUPING, TestFiles.UnfinishedData.SECOND_SET),
#     ])
#     def test_converts_csv_mapping_data_into_deck_part_format(self, test_file, unfinished_data):
#         global_config = get_global_config()
#
#         deck_parts = get_header_model_notes_mock()
#         source_to_deck_parts_returns = [JsonFile(unfinished_data).get_data()]
#
#         expected_result = DeckPartNotes(test_file, deck_parts.note_models).get_data()
#
#         source = setup_source_csv_collection(source_to_deck_parts_returns, read_now=True)
#         headers, note_models, notes = source.source_to_deck_parts()
#
#         assert headers is None
#         assert note_models is None
#
#         # debug_write_to_target_json(notes, test_file)
#
#         assert expected_result == notes
#
#     def test_combines_multiple_csv_mappings(self):
#         global_config = get_global_config()
#
#         source_to_deck_parts_returns = [
#             JsonFile(TestFiles.UnfinishedData.FIRST_SET_SPLIT1).get_data(),
#             JsonFile(TestFiles.UnfinishedData.FIRST_SET_SPLIT2).get_data(),
#         ]
#
#         expected_result = DeckPartNotes(TestFiles.NoteFiles.WITH_SHARED_TAGS_EMPTY_AND_GROUPING).get_data()
#
#         source = setup_source_csv_collection(source_to_deck_parts_returns, read_now=True)
#         headers, note_models, notes = source.source_to_deck_parts()
#
#         assert expected_result == source.notes.get_data()


class TestDeckPartsToSource:
    pass
