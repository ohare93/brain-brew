from dataclasses import dataclass

from brain_brew.representation.yaml.my_yaml import YamlRepr


@dataclass
class Headers(YamlRepr):
    data: dict

    def encode(self) -> dict:
        return self.data
