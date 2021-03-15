from abc import ABC, abstractmethod
from pathlib import Path

from ruamel.yaml import YAML

from brain_brew.utils import create_path_if_not_exists

yaml_load = YAML(typ='safe')


yaml_dump = YAML()
yaml_dump.preserve_quotes = False
yaml_dump.indent(mapping=2, sequence=2, offset=0)
yaml_dump.representer.ignore_aliases = lambda *data: True
# yaml.sort_base_mapping_type_on_output = False


class YamlObject(ABC):
    @staticmethod
    def read_to_dict(filename: str):
        filename = YamlObject.to_filename_yaml(filename)

        if not Path(filename).is_file():
            raise FileNotFoundError(filename)

        with open(filename) as file:
            return yaml_load.load(file)

    @staticmethod
    def to_filename_yaml(filename: str):
        if filename[-5:] != ".yaml" and filename[-4:] != ".yml":
            return filename + ".yaml"
        return filename

    @abstractmethod
    def encode(self) -> dict:
        pass

    @classmethod
    @abstractmethod
    def from_yaml_file(cls, filename: str) -> 'YamlObject':
        pass

    def dump_to_yaml(self, filepath, override_data: dict = None):
        filepath = YamlObject.to_filename_yaml(filepath)

        create_path_if_not_exists(filepath)

        with open(filepath, 'w') as fp:
            yaml_dump.dump(override_data or self.encode(), fp)

