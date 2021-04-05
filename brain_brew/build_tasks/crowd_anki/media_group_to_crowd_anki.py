from dataclasses import dataclass
from typing import Union, List, Set

from brain_brew.configuration.part_holder import PartHolder
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.interfaces.yamale_verifyable import YamlRepr
from brain_brew.representation.generic.media_file import MediaFile
from brain_brew.representation.yaml.media_group import MediaGroup
from brain_brew.transformers.save_media_group_to_location import save_media_groups_to_location


@dataclass
class MediaGroupToCrowdAnki(YamlRepr):
    @classmethod
    def task_name(cls) -> str:
        return r'media_group_to_crowd_anki'

    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            parts: list(str())
        '''

    @dataclass
    class Representation(RepresentationBase):
        parts: List[str]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            rep=rep,
            parts=list(holder.part for holder in map(PartHolder.from_file_manager, rep.parts))
        )

    rep: Representation
    parts: List[MediaGroup]

    def execute(self, ca_media_folder: str) -> Set[MediaFile]:
        return save_media_groups_to_location(self.parts, ca_media_folder, True, False)
