from dataclasses import dataclass
from typing import Union, List, Set

from brain_brew.interfaces.yamale_verifyable import YamlRepr
from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.representation.generic.media_file import MediaFile
from brain_brew.representation.yaml.media_group_repr import MediaGroup
from brain_brew.representation.yaml.part_holder import PartHolder
from brain_brew.transformers.media_group_save_to_location import save_media_groups_to_location


@dataclass
class MediaGroupToCrowdAnki(YamlRepr):
    @classmethod
    def task_regex(cls) -> str:
        return r'media_group_to_crowd_anki'

    @classmethod
    def yamale_validator_and_deps(cls) -> (str, set):
        return f'''\
            {cls.task_regex()}
              parts: list(str())
        ''', None

    @dataclass
    class Representation(RepresentationBase):
        parts: List[str]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            parts=list(map(PartHolder.from_file_manager, rep.parts))
        )

    parts: List[MediaGroup]

    def execute(self, ca_media_folder: str) -> Set[MediaFile]:
        return save_media_groups_to_location(self.parts, ca_media_folder, True)
