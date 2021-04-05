from dataclasses import dataclass, field
from typing import Optional, Union, List

from brain_brew.commands.run_recipe.build_task import BuildPartTask
from brain_brew.configuration.part_holder import PartHolder
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.representation.yaml.media_group import MediaGroup
from brain_brew.transformers.create_media_group_from_location import create_media_group_from_location


@dataclass
class MediaGroupFromFolder(BuildPartTask):
    @classmethod
    def task_name(cls) -> str:
        return r"media_group_from_folder"

    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            part_id: str()
            source: str()
            save_to_file: str(required=False)
            recursive: bool(required=False)
            filter_whitelist_from_parts: list(str(), required=False)
            filter_blacklist_from_parts: list(str(), required=False)
        '''

    @dataclass
    class Representation(RepresentationBase):
        part_id: str
        source: str
        filter_blacklist_from_parts: List[str] = field(default_factory=list)
        filter_whitelist_from_parts: List[str] = field(default_factory=list)
        recursive: Optional[bool] = field(default=True)
        save_to_file: Optional[str] = field(default=None)

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            rep=rep,
            part=create_media_group_from_location(
                part_id=rep.part_id,
                save_to_file=rep.save_to_file,
                media_group=MediaGroup.from_directory(rep.source, rep.recursive),
                groups_to_blacklist=list(holder.part for holder in
                                         map(PartHolder.from_file_manager, rep.filter_blacklist_from_parts)),
                groups_to_whitelist=list(holder.part for holder in
                                         map(PartHolder.from_file_manager, rep.filter_whitelist_from_parts))
                # match criteria
            )
        )

    rep: Representation
    part: MediaGroup

    def execute(self):
        pass
