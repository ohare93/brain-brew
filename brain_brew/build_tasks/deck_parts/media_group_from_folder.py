from dataclasses import dataclass, field
from typing import Optional, Union, List

from brain_brew.representation.build_config.build_task import BuildPartTask
from brain_brew.representation.configuration.representation_base import RepresentationBase
from brain_brew.representation.yaml.media_group_repr import MediaGroup
from brain_brew.representation.yaml.part_holder import PartHolder
from brain_brew.transformers.media_group_from_location import create_media_group_from_location


@dataclass
class MediaGroupFromFolder(BuildPartTask):
    @classmethod
    def task_name(cls) -> str:
        return r"media_group_from_folder"

    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            part_id: str()
            save_to_file: str(required=False)
            source: str()
            recursive: bool(required=False)
            filter_whitelist_from_parts: list(str(), required=False)
            filter_blacklist_from_parts: list(str(), required=False)
        '''

    @dataclass
    class Representation(RepresentationBase):
        source: str
        part_id: str
        filter_blacklist_from_parts: List[str] = field(default_factory=list)
        filter_whitelist_from_parts: List[str] = field(default_factory=list)
        recursive: Optional[bool] = field(default=True)
        save_to_file: Optional[str] = field(default=None)

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
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

    part: MediaGroup

    def execute(self):
        pass
