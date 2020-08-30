import logging
from pathlib import Path
import os
from ruamel.yaml import YAML

yaml_load = YAML(typ='safe')


yaml_dump = YAML()
yaml_dump.preserve_quotes = False
yaml_dump.indent(mapping=2, sequence=2, offset=0)
yaml_dump.representer.ignore_aliases = lambda *data: True
# yaml.sort_base_mapping_type_on_output = False


class YamlRepr:
    @staticmethod
    def read_to_dict(filename: str):
        filename = YamlRepr.append_yaml_if_needed(filename)

        if not Path(filename).is_file():
            raise FileNotFoundError(filename)

        with open(filename) as file:
            return yaml_load.load(file)

    def encode(self) -> dict:
        raise NotImplemented

    def dump_to_yaml(self, filepath):
        filepath = YamlRepr.append_yaml_if_needed(filepath)

        if not Path(filepath).is_file():
            logging.warning(f"Creating missing filepath '{filepath}'")
            os.makedirs(os.path.dirname(filepath), exist_ok=True)

        with open(filepath, 'w') as fp:
            yaml_dump.dump(self.encode(), fp)

    @staticmethod
    def append_yaml_if_needed(filename: str):
        if filename[-5:] != ".yaml" and filename[-4:] != ".yml":
            return filename + ".yaml"
        return filename
