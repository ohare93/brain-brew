from dataclasses import dataclass, field
from typing import Union, Optional

from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.representation.yaml.yaml_object import YamlObject


@dataclass
class GlobalConfig(YamlObject):
    __instance = None

    def encode(self) -> dict:
        pass

    @dataclass
    class Representation(RepresentationBase):
        sort_case_insensitive: Optional[bool] = field(default=False)
        join_values_with: Optional[str] = field(default=" ")

    sort_case_insensitive: bool
    join_values_with: str

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            sort_case_insensitive=rep.sort_case_insensitive,
            join_values_with=rep.join_values_with
        )

    def __post_init__(self):
        if GlobalConfig.__instance is None:
            GlobalConfig.__instance = self
        else:
            raise Exception("Multiple GlobalConfigs created")

    @classmethod
    def from_yaml_file(cls, filename: str = "brain_brew_config.yaml") -> 'GlobalConfig':
        return cls.from_repr(cls.read_to_dict(filename))

    @classmethod
    def get_instance(cls) -> 'GlobalConfig':
        return cls.__instance

    @classmethod
    def clear_instance(cls):
        if cls.__instance:
            cls.__instance = None
