from dataclasses import dataclass
from typing import Optional

from brain_brew.build_tasks.deck_parts.media_group_from_folder import MediaGroupFromFolder
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.representation.yaml.media_group_repr import MediaGroup


@dataclass
class MediaGroupFromCrowdAnki(MediaGroupFromFolder):
    @classmethod
    def task_regex(cls) -> str:
        return r"media_group_from_crowd_anki"

    @classmethod
    def get_source(cls, source: str, recursive: Optional[bool]) -> MediaGroup:
        cae: CrowdAnkiExport = CrowdAnkiExport.create_or_get(source)
        return MediaGroup.from_directory(cae.media_loc, recursive)
