from dataclasses import dataclass, field
from typing import Optional, Union, List

from brain_brew.interfaces.media_container import MediaContainer
from brain_brew.representation.build_config.build_task import PartBuildTask
from brain_brew.representation.configuration.base_parts_from import BasePartsFrom
from brain_brew.representation.yaml.media_group_repr import MediaGroup
from brain_brew.representation.yaml.part_holder import PartHolder
from brain_brew.transformers.media_group_from_location import create_media_group_from_location


@dataclass
class MediaGroupFromFolder(BasePartsFrom, PartBuildTask):
    @classmethod
    def task_regex(cls) -> str:
        return r"media_group_from_folder"

    @classmethod
    def yamale_validator_and_deps(cls) -> (str, set):
        return f'''\
            {cls.task_regex()}:
              part_id: str()
              save_to_file: str(required=False)
              source: str()
              recursive: bool(required=False)
              filter_whitelist_from_parts: list(str(), required=False)
              filter_blacklist_from_parts: list(str(), required=False)
        ''', None

    @dataclass(init=False)
    class Representation(BasePartsFrom.Representation):
        source: str
        recursive: Optional[bool] = field(default=True)
        filter_blacklist_from_parts: List[str] = field(default_factory=[])
        filter_whitelist_from_parts: List[str] = field(default_factory=[])

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            part_id=rep.part_id,
            save_to_file=rep.save_to_file,
            media_group=MediaGroup.from_directory(rep.source, rep.recursive),
            groups_to_blacklist=list(map(PartHolder.from_file_manager, rep.filter_blacklist_from_parts)),
            groups_to_whitelist=list(map(PartHolder.from_file_manager, rep.filter_whitelist_from_parts))
            # match criteria
        )

    media_group: MediaGroup
    groups_to_blacklist: List[MediaContainer]
    groups_to_whitelist: List[MediaContainer]
    # match criteria

    def execute(self):
        create_media_group_from_location(self.part_id, self.save_to_file, self.media_group,
                                         self.groups_to_blacklist, self.groups_to_whitelist)
