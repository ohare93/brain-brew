import logging

from brain_brew.argument_reader import BBArgumentReader
from brain_brew.representation.build_config.top_level_task_builder import TopLevelTaskBuilder
from brain_brew.file_manager import FileManager
from brain_brew.representation.configuration.global_config import GlobalConfig
from brain_brew.representation.generic.yaml_file import YamlFile


# sys.path.append(os.path.join(os.path.dirname(__file__), "dist"))
# sys.path.append(os.path.dirname(__file__))


def main():
    logging.basicConfig(level=logging.DEBUG)

    # Read in Arguments
    argument_reader = BBArgumentReader()
    builder_file_name, global_config_file = argument_reader.get_parsed()
    builder_config = YamlFile.read_file(builder_file_name)

    # Read in Global Config File
    global_config = GlobalConfig.from_yaml(global_config_file) if global_config_file else GlobalConfig.get_default()
    file_manager = FileManager()

    # Parse Build Config File
    builder = TopLevelTaskBuilder.from_dict(builder_config, global_config, file_manager)

    # If all good, execute it
    builder.execute()


if __name__ == "__main__":
    main()
