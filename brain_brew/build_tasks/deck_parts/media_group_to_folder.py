from dataclasses import dataclass, field
from typing import List, Union, Optional

from brain_brew.commands.run_recipe.build_task import BuildPartTask
from brain_brew.configuration.part_holder import PartHolder
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.representation.yaml.media_group import MediaGroup
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
            recursive: bool(required=False)
        '''

    @dataclass
    class Representation(RepresentationBase):
        parts: List[str]
        folder: str
        clear_folder: Optional[bool] = field(default=None)
        recursive: Optional[bool] = field(default=None)

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            parts=list(holder.part for holder in map(PartHolder.from_file_manager, rep.parts)),
            folder=rep.folder,
            clear_folder=rep.clear_folder or False,
            recursive=rep.recursive or False
        )

    parts: List[MediaGroup]
    folder: str
    clear_folder: bool
    recursive: bool

    def execute(self):
        save_media_groups_to_location(self.parts, self.folder, self.clear_folder, self.recursive)
