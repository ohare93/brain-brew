from pathlib import Path

from ruamel.yaml import YAML

yaml_load = YAML(typ='safe')


yaml_dump = YAML()
yaml_dump.preserve_quotes = False
yaml_dump.indent(mapping=0, sequence=4, offset=2)
yaml_dump.representer.ignore_aliases = lambda *data: True
# yaml.sort_base_mapping_type_on_output = False


class YamlRepr:
    @staticmethod
    def read_to_dict(filename: str):
        if filename[-5:] not in [".yaml", ".yml"]:
            filename += ".yaml"

        if not Path(filename).is_file():
            raise FileNotFoundError(filename)

        with open(filename) as file:
            return yaml_load.load(file)

    def encode(self) -> dict:
        raise NotImplemented

    def dump_to_yaml(self, filepath):
        with open(filepath, 'w+') as fp:  # TODO: raise warning/log if file not exists
            yaml_dump.dump(self.encode(), fp)
