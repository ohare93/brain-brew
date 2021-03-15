import os
from dataclasses import dataclass
from typing import List

from brain_brew.build_tasks.crowd_anki.headers_from_crowdanki import HeadersFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.media_group_from_crowd_anki import MediaGroupFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.note_models_all_from_crowd_anki import NoteModelsAllFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.notes_from_crowd_anki import NotesFromCrowdAnki
from brain_brew.build_tasks.csvs.csvs_generate import CsvsGenerate
from brain_brew.build_tasks.deck_parts.media_group_from_folder import MediaGroupFromFolder
from brain_brew.build_tasks.deck_parts.save_media_group_to_folder import SaveMediaGroupsToFolder
from brain_brew.build_tasks.deck_parts.save_note_models_to_folder import SaveNoteModelsToFolder
from brain_brew.interfaces.command import Command
from brain_brew.representation.generic.csv_file import CsvFile
from brain_brew.transformers.file_mapping import FileMapping
from brain_brew.transformers.note_model_mapping import NoteModelMapping
from brain_brew.utils import create_path_if_not_exists

RECIPE_MEDIA = "deck_media"
RECIPE_HEADERS = "deck_headers"
RECIPE_NOTES = "deck_notes"

LOC_RECIPES = "recipes/"
LOC_BUILD = "build/"
LOC_DATA = "src/data/"
LOC_HEADERS = "src/headers/"
LOC_NOTE_MODELS = "src/note_models/"
LOC_MEDIA = "src/media/"


@dataclass
class InitRepo(Command):
    crowdanki_folder: str

    def execute(self):
        self.setup_repo_structure()

        # Headers
        headers_from_crowdanki = HeadersFromCrowdAnki.from_repr(HeadersFromCrowdAnki.Representation(
            source=self.crowdanki_folder, part_id=RECIPE_HEADERS))

        headers = headers_from_crowdanki.execute().part
        headers.dump_to_yaml(LOC_HEADERS + "header1.yaml")
        # TODO: desc file

        # Note Models
        note_models_all_from_crowdanki = NoteModelsAllFromCrowdAnki.from_repr(NoteModelsAllFromCrowdAnki.Representation(
            source=self.crowdanki_folder))

        note_models = [m.part for m in note_models_all_from_crowdanki.execute()]
        save_note_models_to_folder = SaveNoteModelsToFolder(note_models, LOC_NOTE_MODELS, True)
        model_yaml_files = save_note_models_to_folder.execute()

        # Notes
        notes_from_crowdanki = NotesFromCrowdAnki.from_repr(NotesFromCrowdAnki.Representation(
            source=self.crowdanki_folder, part_id=RECIPE_NOTES))
        notes_from_crowdanki.execute()

        note_model_mappings: List[NoteModelMapping.Representation] = []
        file_mappings: List[FileMapping.Representation] = []

        for model in note_models:
            mapping_rep = NoteModelMapping.Representation.from_note_model(model)
            csv_file_path = os.path.join(LOC_DATA, CsvFile.to_filename_csv(model.name))
            CsvFile.create_file_with_headers(csv_file_path, list(mapping_rep.columns_to_fields.keys()))

            note_model_mappings.append(mapping_rep)
            file_mappings.append(FileMapping.Representation(
                file=csv_file_path,
                note_model=model.name,
                sort_by_columns=['guid']
            ))

        generate_csvs = CsvsGenerate.from_repr({
            'notes': RECIPE_NOTES,
            'note_model_mappings': note_model_mappings,
            'file_mappings': file_mappings
        })
        generate_csvs.execute()

        # Media
        MediaGroupFromCrowdAnki.from_repr(MediaGroupFromFolder.Representation(
            source=self.crowdanki_folder, part_id=RECIPE_MEDIA
        ))
        save_media_to_folder = SaveMediaGroupsToFolder.from_repr(SaveMediaGroupsToFolder.Representation(
            parts=[RECIPE_MEDIA], folder=LOC_MEDIA, recursive=True, clear_folder=True
        ))
        save_media_to_folder.execute()

        # Recipes
        # Todo


    def setup_repo_structure(self):
        create_path_if_not_exists(LOC_RECIPES)
        create_path_if_not_exists(LOC_BUILD)
        create_path_if_not_exists(LOC_DATA)
        create_path_if_not_exists(LOC_HEADERS)
        create_path_if_not_exists(LOC_NOTE_MODELS)
        create_path_if_not_exists(LOC_MEDIA)


