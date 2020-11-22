from brain_brew.representation.build_config.top_level_builder import TopLevelBuilder
from brain_brew.representation.yaml.yaml_object import YamlObject


class YamaleBuildFile(YamlObject):
    def encode(self) -> dict:
        TopLevelBuilder.build_yamale()

    @classmethod
    def from_yaml_file(cls, filename: str) -> 'YamlObject':
        pass


