from dataclasses import dataclass
from typing import List, Union, Optional

from brain_brew.representation.build_config.build_task import PartBuildTask
from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.representation.yaml.media_group_repr import MediaGroup
from brain_brew.representation.yaml.part_holder import PartHolder
from brain_brew.transformers.media_group_save_to_location import save_media_groups_to_location


@dataclass
class MediaGroupsToFolder(PartBuildTask):
    @classmethod
    def task_regex(cls) -> str:
        return r"save_media_groups_to_folder"

    @classmethod
    def yamale_validator_and_deps(cls) -> (str, set):
        return f'''\
            {cls.task_regex()}
              parts: list(str())
              folder: str()
              clear_folder: bool(required=False)
        ''', None

    @dataclass
    class Representation(RepresentationBase):
        parts: List[str]
        folder: str
        clear_folder: Optional[bool]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            parts=list(map(PartHolder.from_file_manager, rep.parts)),
            folder=rep.folder,
            clear_folder=rep.clear_folder or False
        )

    parts: List[MediaGroup]
    folder: str
    clear_folder: bool

    def execute(self):
        save_media_groups_to_location(self.parts, self.folder, self.clear_folder)
