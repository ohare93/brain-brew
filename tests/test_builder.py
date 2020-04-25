from unittest.mock import patch

from brain_brew.build_tasks.source_crowd_anki import SourceCrowdAnki
from brain_brew.build_tasks.source_csv import SourceCsv
from brain_brew.builder import Builder
from brain_brew.representation.generic.yaml_file import YamlFile
from tests.test_files import TestFiles
from tests.representation.configuration.test_global_config import global_config


class TestConstructor:
    def test_runs(self, global_config):

        with patch.object(SourceCsv, "__init__", return_value=None) as mock_csv, \
                patch.object(SourceCsv, "verify_contents", return_value=None), \
                patch.object(SourceCrowdAnki, "__init__", return_value=None) as mock_ca:

            data = YamlFile.read_file(TestFiles.BuildConfig.ONE_OF_EACH_TYPE)
            builder = Builder(data, global_config, None, read_now=False)

            assert len(builder.build_tasks) == 2
            assert mock_csv.call_count == 1
            assert mock_ca.call_count == 1
