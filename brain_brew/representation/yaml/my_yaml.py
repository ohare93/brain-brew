from dataclasses import dataclass
from typing import Optional

from ruamel.yaml import YAML

from brain_brew.file_manager import FileManager

yaml_load = YAML(typ='safe')


yaml_dump = YAML()
yaml_dump.preserve_quotes = False
yaml_dump.indent(mapping=0, sequence=4, offset=2)
yaml_dump.representer.ignore_aliases = lambda *data: True
# yaml.sort_base_mapping_type_on_output = False


@dataclass
class YamlRepresentation:
    name: str
    save_to_file: Optional[str]

    @classmethod
    def from_deck_part_pool(cls, name: str) -> 'YamlRepresentation':
        return FileManager.get_instance().deck_part_from_pool(name)

    def write_to_file(self):
        raise NotImplemented
