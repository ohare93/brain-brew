import logging

from brain_brew.configuration.argument_reader import BBArgumentReader
from brain_brew.configuration.build_config.top_level_builder import TopLevelBuilder
from brain_brew.configuration.file_manager import FileManager
from brain_brew.configuration.global_config import GlobalConfig
# sys.path.append(os.path.join(os.path.dirname(__file__), "dist"))
# sys.path.append(os.path.dirname(__file__))
from brain_brew.configuration.yaml_verifier import YamlVerifier


def main():
    logging.basicConfig(level=logging.DEBUG)

    # Read in Arguments
    argument_reader = BBArgumentReader()
    recipe_file_name, global_config_file, verify_only = argument_reader.get_parsed()

    # Read in Global Config File
    GlobalConfig.from_yaml_file(global_config_file) if global_config_file else GlobalConfig.from_yaml_file()
    FileManager()

    # Parse Build Config File
    YamlVerifier()
    recipe = TopLevelBuilder.parse_and_read(recipe_file_name, verify_only)

    if not verify_only:
        recipe.execute()


if __name__ == "__main__":
    main()
