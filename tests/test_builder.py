from unittest.mock import patch

from brain_brew.build_tasks.generate_guids import GenerateGuids
from brain_brew.build_tasks.source_crowd_anki import SourceCrowdAnki
from brain_brew.build_tasks.source_csv import SourceCsv
from brain_brew.build_tasks.source_csv_collection import SourceCsvCollection
from brain_brew.builder import Builder
from brain_brew.representation.configuration.yaml_file import YamlFile
from tests.test_files import TestFiles
from tests.test_helpers import get_global_config


def setup_builder():
    global_config = get_global_config()
    data = YamlFile.read_file(TestFiles.BuildConfig.ONE_OF_EACH_TYPE)
    builder = Builder(data, global_config, None, read_now=False)
    return builder


class TestConstructor:
    def test_runs(self):

        with patch.object(SourceCsvCollection, "__init__", lambda x, y, z: None), \
             patch.object(SourceCsv, "__init__", lambda x, y, z: None), \
             patch.object(GenerateGuids, "__init__", lambda x, y, z: None), \
             patch.object(SourceCrowdAnki, "__init__", lambda x, y, z: None):

            builder = setup_builder()

            assert len(builder.build_tasks) == 4
