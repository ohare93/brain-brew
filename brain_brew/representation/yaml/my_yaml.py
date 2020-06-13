from ruamel.yaml import YAML

yaml_load = YAML(typ='safe')


yaml_dump = YAML()
yaml_dump.preserve_quotes = False
yaml_dump.indent(mapping=0, sequence=6, offset=4)
yaml_dump.representer.ignore_aliases = lambda *data: True
# yaml.sort_base_mapping_type_on_output = False
