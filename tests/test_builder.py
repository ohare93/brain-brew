from unittest.mock import patch, Mock

from brain_brew.representation.build_config.top_level_task_builder import TopLevelTaskBuilder
from brain_brew.representation.deck_part_transformers.tr_notes_csv_collection import TrNotesToCsvCollection
from brain_brew.representation.yaml.my_yaml import YamlRepresentation
from brain_brew.representation.yaml.note_model import DeckPartNoteModel
from brain_brew.representation.yaml.note_repr import DeckPartNotes
from tests.representation.json.test_deck_part_note_model import mock_dp_nm
from tests.test_file_manager import get_new_file_manager
from tests.test_files import TestFiles


class TestConstructor:
    def test_runs(self, global_config):
        fm = get_new_file_manager()

        with patch.object(TrNotesToCsvCollection, "__init__", return_value=None) as mock_csv_tr, \
                patch.object(DeckPartNotes, "from_deck_part_pool", return_value=Mock()), \
                patch.object(DeckPartNoteModel, "create", side_effect=mock_dp_nm):

            data = YamlRepresentation.filename_to_dict(TestFiles.BuildConfig.ONE_OF_EACH_TYPE)
            builder = TopLevelTaskBuilder.from_dict(data, global_config, fm)

            assert len(builder.tasks) == 1
            assert mock_csv_tr.call_count == 1
