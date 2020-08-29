from dataclasses import dataclass

from brain_brew.representation.yaml.my_yaml import YamlRepr


@dataclass
class Headers(YamlRepr):
    data: dict

    @classmethod
    def from_file(cls, filename: str):
        return cls(data=cls.read_to_dict(filename))

    def encode(self) -> dict:
        return self.data
