import os
from dataclasses import dataclass
from typing import List

from brain_brew.build_tasks.crowd_anki.headers_from_crowdanki import HeadersFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.media_group_from_crowd_anki import MediaGroupFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.note_models_all_from_crowd_anki import NoteModelsAllFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.notes_from_crowd_anki import NotesFromCrowdAnki
from brain_brew.build_tasks.csvs.csvs_generate import CsvsGenerate
from brain_brew.build_tasks.deck_parts.media_group_from_folder import MediaGroupFromFolder
from brain_brew.build_tasks.deck_parts.note_model_from_yaml_part import NoteModelsFromYamlPart
from brain_brew.build_tasks.deck_parts.save_media_group_to_folder import SaveMediaGroupsToFolder
from brain_brew.build_tasks.deck_parts.save_note_models_to_folder import SaveNoteModelsToFolder
from brain_brew.commands.run_recipe.build_task import TopLevelBuildTask, BuildPartTask
from brain_brew.commands.run_recipe.parts_builder import PartsBuilder
from brain_brew.commands.run_recipe.top_level_builder import TopLevelBuilder
from brain_brew.interfaces.command import Command
from brain_brew.representation.generic.csv_file import CsvFile
from brain_brew.representation.yaml.note_model import NoteModel
from brain_brew.representation.yaml.yaml_object import YamlObject
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

    @staticmethod
    def parts_from_crowdanki(folder: str):
        headers_ca = HeadersFromCrowdAnki.from_repr(HeadersFromCrowdAnki.Representation(
            source=folder, part_id=RECIPE_HEADERS
        ))

        note_models_all_ca = NoteModelsAllFromCrowdAnki.from_repr(NoteModelsAllFromCrowdAnki.Representation(
            source=folder
        ))

        notes_ca = NotesFromCrowdAnki.from_repr(NotesFromCrowdAnki.Representation(
            source=folder, part_id=RECIPE_NOTES
        ))

        media_group_ca = MediaGroupFromCrowdAnki.from_repr(MediaGroupFromFolder.Representation(
            source=folder, part_id=RECIPE_MEDIA
        ))

        return headers_ca, note_models_all_ca, notes_ca, media_group_ca

    def execute(self):
        self.setup_repo_structure()

        # Create the Deck Parts used
        headers_ca, note_models_all_ca, notes_ca, media_group_ca = self.parts_from_crowdanki(self.crowdanki_folder)

        headers = headers_ca.execute().part
        headers.dump_to_yaml(LOC_HEADERS + "header1.yaml")
        # TODO: desc file

        note_models = [m.part for m in note_models_all_ca.execute()]

        notes_ca.execute()

        media_group_ca.execute()

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

        deck_path = os.path.join(LOC_BUILD, "deck")  # TODO: name the same as the file provided

        self.create_anki_to_source(deck_path, note_models, note_model_mappings, file_mappings)
        self.create_source_to_anki(deck_path, note_models, note_model_mappings, file_mappings)

        # print(encoded_top_level_tasks)

    def setup_repo_structure(self):
        create_path_if_not_exists(LOC_RECIPES)
        create_path_if_not_exists(LOC_BUILD)
        create_path_if_not_exists(LOC_DATA)
        create_path_if_not_exists(LOC_HEADERS)
        create_path_if_not_exists(LOC_NOTE_MODELS)
        create_path_if_not_exists(LOC_MEDIA)

    def create_anki_to_source(self,
                              deck_path: str, note_models: List[NoteModel],
                              note_model_mappings: List[NoteModelMapping.Representation],
                              file_mappings: List[FileMapping.Representation]):
        # Save Files to Source
        save_note_models_to_folder = SaveNoteModelsToFolder.from_repr(SaveNoteModelsToFolder.Representation(
            [m.name for m in note_models], LOC_NOTE_MODELS, True
        ))
        model_yaml_files = save_note_models_to_folder.execute()

        save_media_to_folder = SaveMediaGroupsToFolder.from_repr(SaveMediaGroupsToFolder.Representation(
            parts=[RECIPE_MEDIA], folder=LOC_MEDIA, recursive=True, clear_folder=True
        ))
        save_media_to_folder.execute()

        generate_csvs = CsvsGenerate.from_repr({
            'notes': RECIPE_NOTES,
            'note_model_mappings': note_model_mappings,
            'file_mappings': file_mappings
        })
        generate_csvs.execute()

        # Create Recipe
        headers_recipe, note_models_all_recipe, notes_recipe, media_group_recipe = self.parts_from_crowdanki(deck_path)

        build_part_tasks: List[BuildPartTask] = [
            headers_recipe,
            notes_recipe,
            note_models_all_recipe,
            media_group_recipe,
        ]
        dp_builder = PartsBuilder(build_part_tasks)

        top_level_tasks: List[TopLevelBuildTask] = [dp_builder, save_media_to_folder, generate_csvs]
        tl_builder = TopLevelBuilder(top_level_tasks)

        encoded_top_level_tasks = tl_builder.encode()

        model_yaml_file_name = YamlObject.to_filename_yaml(os.path.join(LOC_RECIPES, "anki_to_source"))
        YamlObject.dump_to_yaml_file(model_yaml_file_name, encoded_top_level_tasks)

    # def create_source_to_anki(self,
    #                           deck_path: str, note_models: List[NoteModel],
    #                           note_model_mappings: List[NoteModelMapping.Representation],
    #                           file_mappings: List[FileMapping.Representation]):
    #     # Save Files to Source
    #     save_note_models_to_folder = NoteModelsFromYamlPart.from_repr(NoteModelsFromYamlPart.Representation(
    #         [m.name for m in note_models], LOC_NOTE_MODELS, True
    #     ))
    #
    #     save_media_to_folder = SaveMediaGroupsToFolder.from_repr(SaveMediaGroupsToFolder.Representation(
    #         parts=[RECIPE_MEDIA], folder=LOC_MEDIA, recursive=True, clear_folder=True
    #     ))
    #
    #     generate_csvs = CsvsGenerate.from_repr({
    #         'notes': RECIPE_NOTES,
    #         'note_model_mappings': note_model_mappings,
    #         'file_mappings': file_mappings
    #     })
    #
    #     # Create Recipe
    #     headers_recipe, note_models_all_recipe, notes_recipe, media_group_recipe = self.parts_from_crowdanki(deck_path)
    #
    #     build_part_tasks: List[BuildPartTask] = [
    #         headers_recipe,
    #         notes_recipe,
    #         note_models_all_recipe,
    #         media_group_recipe,
    #     ]
    #     dp_builder = PartsBuilder(build_part_tasks)
    #
    #     top_level_tasks: List[TopLevelBuildTask] = [dp_builder, save_media_to_folder, generate_csvs]
    #     tl_builder = TopLevelBuilder(top_level_tasks)
    #
    #     encoded_top_level_tasks = tl_builder.encode()
    #
    #     model_yaml_file_name = YamlObject.to_filename_yaml(os.path.join(LOC_RECIPES, "anki_to_source"))
    #     YamlObject.dump_to_yaml_file(model_yaml_file_name, encoded_top_level_tasks)