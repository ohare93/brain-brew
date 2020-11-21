from typing import List

from brain_brew.interfaces.media_container import MediaContainer
from brain_brew.representation.yaml.part_holder import PartHolder
from brain_brew.representation.yaml.media_group_repr import MediaGroup


def create_media_group_from_location(
        part_id: str,
        save_to_file: str,
        media_group: MediaGroup,
        groups_to_blacklist: List[MediaContainer],
        groups_to_whitelist: List[MediaContainer]
):
    if groups_to_whitelist:
        white = list(set.union(*[container.get_all_media_references() for container in groups_to_whitelist]))
        media_group.filter_by_filenames(white, should_match=True)

    if groups_to_blacklist:
        black = list(set.union(*[container.get_all_media_references() for container in groups_to_blacklist]))
        media_group.filter_by_filenames(black, should_match=False)

    PartHolder.override_or_create(part_id, save_to_file, media_group)
