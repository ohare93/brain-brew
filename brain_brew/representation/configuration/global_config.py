from dataclasses import dataclass, field
from typing import List, Union, Optional

from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.representation.yaml.my_yaml import YamlRepr


@dataclass
class GlobalConfig(YamlRepr):
    __instance = None

    @dataclass
    class Defaults:
        @dataclass
        class Representation(RepresentationBase):
            note_sort_order: Optional[Union[List[str], str]] = field(default_factory=[])
            sort_case_insensitive: Optional[bool] = field(default=False)
            reverse_sort: Optional[bool] = field(default=False)
            join_values_with: Optional[str] = field(default=" ")

        note_sort_order: list
        sort_case_insensitive: bool
        reverse_sort: bool
        join_values_with: str

        @classmethod
        def from_repr(cls, data: Union[Representation, dict]):
            rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
            return cls(
                note_sort_order=rep.note_sort_order,
                sort_case_insensitive=rep.sort_case_insensitive,
                reverse_sort=rep.reverse_sort,
                join_values_with=rep.join_values_with
            )

    @dataclass
    class DeckPartLocations(RepresentationBase):
        headers: str
        note_models: str
        notes: str
        media_files: str

    @dataclass
    class Representation(RepresentationBase):
        deck_parts: dict
        defaults: Optional[dict]

    deck_parts: DeckPartLocations
    defaults: Defaults

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            deck_parts=cls.DeckPartLocations.from_dict(rep.deck_parts),
            defaults=cls.Defaults.from_repr(rep.defaults)
        )

    def __post_init__(self):
        if GlobalConfig.__instance is None:
            GlobalConfig.__instance = self
        else:
            raise Exception("Multiple GlobalConfigs created")

    @classmethod
    def from_file(cls, filename: str = "brain_brew_config.yaml"):
        return cls.from_repr(cls.read_to_dict(filename))

    @classmethod
    def get_instance(cls) -> 'GlobalConfig':
        return cls.__instance

    @classmethod
    def clear_instance(cls):
        if cls.__instance:
            cls.__instance = None
