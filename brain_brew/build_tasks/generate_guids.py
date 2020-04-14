from brain_brew.build_tasks.build_task_generic import BuildTaskGeneric, BuildConfigKeys, BuildTaskEnum
from brain_brew.build_tasks.source_csv import SourceCsvKeys
from brain_brew.build_tasks.source_csv_collection import SourceCsvCollection
from brain_brew.representation.configuration.yaml_file import YamlFile, ConfigKey


class GenerateGuids(BuildTaskGeneric, YamlFile):
    @staticmethod
    def get_build_keys():
        return [
            BuildTaskEnum("generate_guids", GenerateGuids, "execute", "execute")
        ]

    config_entry = {}
    expected_keys = {
        SourceCsvKeys.CSV_FILE.value: ConfigKey(False, (str, list), None),
        BuildConfigKeys.SUBCONFIG.value: ConfigKey(False, list, None)
    }
    subconfig_filter = ["csv"]

    csv_collection: SourceCsvCollection

    def __init__(self, config_data: dict, read_now=True):
        self.setup_config_with_subconfig_replacement(config_data)
        self.verify_config_entry()

        # source_type = self.config_entry[BuildConfigKeys.SOURCE.value][BuildConfigKeys.SOURCE_TYPE.value]
        # if source_type != "csv":
        #     raise TypeError(f"Wrong type of source given to GenerateGuids task. Expected csv but got {source_type}")
        #
        # csv_collection_config =
        #       self.config_entry[BuildConfigKeys.SOURCE.value][BuildConfigKeys.SOURCE_CONFIG_FILE.value]
        # self.csv_collection = SourceCsvCollection.from_yaml(csv_collection_config, read_now=read_now)

    def execute(self):
        pass
