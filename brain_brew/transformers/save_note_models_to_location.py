import logging
import os
from typing import List

from brain_brew.representation.yaml.note_model import NoteModel
from brain_brew.utils import create_path_if_not_exists, clear_contents_of_folder


def save_note_models_to_location(
        parts: List[NoteModel],
        folder: str,
        clear_folder: bool
):

    for model in parts:
        nm_folder = os.path.join(folder, model.name)
        create_path_if_not_exists(nm_folder)

        if clear_folder:
            clear_contents_of_folder(nm_folder)

        model_encoded = model.encode_as_part_with_file_references(folder)

        print(model_encoded)

        # Save their inner files
            # Templates
            # CSS

        # Save the Yaml pointing to each

