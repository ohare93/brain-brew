from dataclasses import dataclass, field
from typing import Optional, Union, List

from brain_brew.interfaces.media_container import MediaContainer
from brain_brew.representation.transformers.base_deck_part_from import BaseDeckPartsFrom
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.media_group_repr import MediaGroup


@dataclass
class MediaGroupFromFolder(BaseDeckPartsFrom):
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
        '''

    @dataclass
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
            media_in_folder=MediaGroup.from_directory(rep.source, rep.recursive),
            groups_to_blacklist=list(map(DeckPartHolder.from_deck_part_pool, rep.filter_blacklist_from_parts)),
            groups_to_whitelist=list(map(DeckPartHolder.from_deck_part_pool, rep.filter_whitelist_from_parts))
            # match criteria
        )

    media_in_folder: MediaGroup
    groups_to_blacklist: List[MediaContainer]
    groups_to_whitelist: List[MediaContainer]
    # match criteria

    def execute(self):

        self.media_in_folder.compare_media_containers()
        # Find matching media files from match criteria


