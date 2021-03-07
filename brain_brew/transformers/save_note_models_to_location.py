import logging
from typing import List, Set

from brain_brew.representation.generic.media_file import MediaFile
from brain_brew.representation.yaml.media_group import MediaGroup
from brain_brew.representation.yaml.note_model import NoteModel
from brain_brew.utils import create_path_if_not_exists


def save_note_models_to_location(
        parts: List[NoteModel],
        folder: str,
        clear_folder: bool
):

    create_path_if_not_exists(folder)

    for model in parts:
        # Save their inner files
            # Templates
            # CSS

        # Save the Yaml pointing to each

        pass