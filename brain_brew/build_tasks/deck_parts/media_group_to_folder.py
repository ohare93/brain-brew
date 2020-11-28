from dataclasses import dataclass
from typing import List, Union, Optional

from brain_brew.representation.build_config.build_task import BuildPartTask
from brain_brew.representation.configuration.representation_base import RepresentationBase
from brain_brew.representation.yaml.media_group_repr import MediaGroup
from brain_brew.representation.yaml.part_holder import PartHolder
from brain_brew.transformers.media_group_save_to_location import save_media_groups_to_location


@dataclass
class MediaGroupsToFolder(BuildPartTask):
    @classmethod
    def task_name(cls) -> str:
        return r'save_media_groups_to_folder'

    @classmethod
    def task_regex(cls) -> str:
        return r"save_media_group[s]?_to_folder"

    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            parts: list(str())
            folder: str()
            clear_folder: bool(required=False)
        '''

    @dataclass
    class Representation(RepresentationBase):
        parts: List[str]
        folder: str
        clear_folder: Optional[bool]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            parts=list(holder.part for holder in map(PartHolder.from_file_manager, rep.parts)),
            folder=rep.folder,
            clear_folder=rep.clear_folder or False
        )

    parts: List[MediaGroup]
    folder: str
    clear_folder: bool

    def execute(self):
        save_media_groups_to_location(self.parts, self.folder, self.clear_folder)
