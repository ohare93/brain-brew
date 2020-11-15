from dataclasses import dataclass, field
from typing import Optional, Union, List

from brain_brew.interfaces.media_container import MediaContainer
from brain_brew.representation.build_config.build_task import DeckPartBuildTask
from brain_brew.representation.transformers.base_deck_part_from import BaseDeckPartsFrom
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.media_group_repr import MediaGroup


@dataclass
class MediaGroupFromFolder(BaseDeckPartsFrom, DeckPartBuildTask):
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

    @classmethod
    def get_source(cls, source: str, recursive: Optional[bool]) -> MediaGroup:
        return MediaGroup.from_directory(source, recursive)

    @dataclass(init=False)
    class Representation(BaseDeckPartsFrom.Representation):
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
            media_group=cls.get_source(rep.source, rep.recursive),
            groups_to_blacklist=list(map(DeckPartHolder.from_file_manager, rep.filter_blacklist_from_parts)),
            groups_to_whitelist=list(map(DeckPartHolder.from_file_manager, rep.filter_whitelist_from_parts))
            # match criteria
        )

    media_group: MediaGroup
    groups_to_blacklist: List[MediaContainer]
    groups_to_whitelist: List[MediaContainer]
    # match criteria

    def execute(self):
        if self.groups_to_whitelist:
            white = list(set.union(*[container.get_all_media_references() for container in self.groups_to_whitelist]))
            self.media_group.filter_by_filenames(white, should_match=True)

        if self.groups_to_blacklist:
            black = list(set.union(*[container.get_all_media_references() for container in self.groups_to_blacklist]))
            self.media_group.filter_by_filenames(black, should_match=False)

        DeckPartHolder.override_or_create(self.part_id, self.save_to_file, self.media_group)
