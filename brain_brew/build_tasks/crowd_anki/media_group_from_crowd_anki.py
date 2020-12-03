from dataclasses import dataclass
from typing import Union

from brain_brew.build_tasks.deck_parts.media_group_from_folder import MediaGroupFromFolder
from brain_brew.configuration.part_holder import PartHolder
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.representation.yaml.media_group import MediaGroup
from brain_brew.transformers.media_group_from_location import create_media_group_from_location


@dataclass
class MediaGroupFromCrowdAnki(MediaGroupFromFolder):
    @classmethod
    def task_name(cls) -> str:
        return r"media_group_from_crowd_anki"

    @classmethod
    def from_repr(cls, data: Union[MediaGroupFromFolder.Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)

        cae: CrowdAnkiExport = CrowdAnkiExport.create_or_get(rep.source)
        return cls(
            part=create_media_group_from_location(
                part_id=rep.part_id,
                save_to_file=rep.save_to_file,
                media_group=MediaGroup.from_directory(cae.media_loc, rep.recursive),
                groups_to_blacklist=list(holder.part for holder in
                                         map(PartHolder.from_file_manager, rep.filter_blacklist_from_parts)),
                groups_to_whitelist=list(holder.part for holder in
                                         map(PartHolder.from_file_manager, rep.filter_whitelist_from_parts))
            )
        )

    part: MediaGroup

    def execute(self):
        pass
