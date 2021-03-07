import logging
from typing import List, Set

from brain_brew.representation.generic.media_file import MediaFile
from brain_brew.representation.yaml.media_group import MediaGroup
from brain_brew.utils import create_path_if_not_exists


def save_media_groups_to_location(
        parts: List[MediaGroup],
        folder: str,
        clear_folder: bool,
        recursive: bool
) -> Set[MediaFile]:

    existing_media_group = MediaGroup.from_directory(folder, recursive)
    all_media_group = MediaGroup.from_many(parts)

    in_both, to_move, to_delete = all_media_group.compare(existing_media_group)

    create_path_if_not_exists(folder)
    for filename, media_file in all_media_group.media_files.items():
        if filename in in_both:
            media_file.copy_self_to_target(existing_media_group.media_files[filename].file_path)
            # TODO: Check if copying is needed?
        elif filename in to_move:
            media_file.copy_self_to_target(folder)

    if clear_folder and to_delete:
        deleted = '\n'.join(to_delete)
        logging.warning(f"Deleting extra files in media folder '{folder}':\n{'-'*20}\n{deleted}\n{'-'*20}")
        for delete_name in to_delete:
            existing_media_group.media_files[delete_name].delete_self()

    return set(all_media_group.media_files.values())
