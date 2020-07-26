from unittest.mock import patch

from brain_brew.representation.build_config.top_level_task_builder import TopLevelTaskBuilder
from brain_brew.representation.deck_part_transformers.tr_notes_csv_collection import TrNotesToCsvCollection
from brain_brew.representation.generic.yaml_file import YamlFile
from tests.test_file_manager import get_new_file_manager
from tests.test_files import TestFiles
from tests.representation.configuration.test_global_config import global_config


class TestConstructor:
    def test_runs(self, global_config):
        fm = get_new_file_manager()

        with patch.object(TrNotesToCsvCollection, "__init__", return_value=None) as mock_csv_tr:

            data = YamlFile.read_file(TestFiles.BuildConfig.ONE_OF_EACH_TYPE)
            builder = TopLevelTaskBuilder.from_dict(data, global_config, fm)

            assert len(builder.build_tasks) == 1
            assert mock_csv_tr.call_count == 1
