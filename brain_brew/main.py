import os
import sys
import logging

# sys.path.append(os.path.join(os.path.dirname(__file__), "dist"))
# sys.path.append(os.path.dirname(__file__))

from brain_brew.builder import Builder
from brain_brew.representation.configuration.global_config import GlobalConfig
from brain_brew.argument_reader import ArgumentReader
from brain_brew.representation.configuration.yaml_file import YamlFile


def main():
    logging.basicConfig(filename='log.txt', level=logging.DEBUG)
    print("Running Main")

    # Read in Global Config File
    global_config = GlobalConfig.get_instance()

    # Read in Arguments
    argument_reader = ArgumentReader()
    builder_file_name, other_arguments = argument_reader.get_parsed()
    builder_config = YamlFile.read_file(builder_file_name)

    # Run chosen Builder
    builder = Builder(builder_config, global_config, other_arguments)
    builder.execute()
