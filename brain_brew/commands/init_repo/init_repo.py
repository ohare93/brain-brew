import os
from dataclasses import dataclass
from typing import List

from brain_brew.build_tasks.crowd_anki.crowd_anki_generate import CrowdAnkiGenerate
from brain_brew.build_tasks.crowd_anki.headers_from_crowdanki import HeadersFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.headers_to_crowd_anki import HeadersToCrowdAnki
from brain_brew.build_tasks.crowd_anki.media_group_from_crowd_anki import MediaGroupFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.media_group_to_crowd_anki import MediaGroupToCrowdAnki
from brain_brew.build_tasks.crowd_anki.note_models_all_from_crowd_anki import NoteModelsAllFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.note_models_to_crowd_anki import NoteModelsToCrowdAnki
from brain_brew.build_tasks.crowd_anki.notes_from_crowd_anki import NotesFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.notes_to_crowd_anki import NotesToCrowdAnki
from brain_brew.build_tasks.csvs.csvs_generate import CsvsGenerate
from brain_brew.build_tasks.csvs.generate_guids_in_csvs import GenerateGuidsInCsvs
from brain_brew.build_tasks.csvs.notes_from_csvs import NotesFromCsvs
from brain_brew.build_tasks.deck_parts.headers_from_yaml_part import HeadersFromYamlPart
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
from brain_brew.utils import create_path_if_not_exists, filename_from_full_path, folder_name_from_full_path

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

        # Create the Deck Parts used
        headers_ca, note_models_all_ca, notes_ca, media_group_ca = self.parts_from_crowdanki(self.crowdanki_folder)

        headers = headers_ca.execute().part
        headers_name = LOC_HEADERS + "header1.yaml"
        headers.dump_to_yaml(headers_name)
        # TODO: desc file

        note_models = [m.part for m in note_models_all_ca.execute()]

        notes = notes_ca.execute().part
        used_note_models_in_notes = notes.get_all_known_note_model_names()

        media_group_ca.execute()

        note_model_mappings = [NoteModelMapping.Representation([model.name for model in note_models])]
        file_mappings: List[FileMapping.Representation] = []

        csv_files = []

        for model in note_models:
            if model.name in used_note_models_in_notes:
                csv_file_path = os.path.join(LOC_DATA, CsvFile.to_filename_csv(model.name))
                column_headers = ["guid"] + model.field_names_lowercase + ["tags"]
                CsvFile.create_file_with_headers(csv_file_path, column_headers)

                file_mappings.append(FileMapping.Representation(
                    file=csv_file_path,
                    note_model=model.name
                ))

                csv_files.append(csv_file_path)

        deck_path = os.path.join(LOC_BUILD, folder_name_from_full_path(self.crowdanki_folder))

        # Generate the Source files that will be kept in the repo
        save_note_models_to_folder = SaveNoteModelsToFolder.from_repr(SaveNoteModelsToFolder.Representation(
            [m.name for m in note_models], LOC_NOTE_MODELS, True
        ))
        model_name_to_file_dict = save_note_models_to_folder.execute()

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

        # Create Recipes

        # Anki to Source
        headers_recipe, note_models_all_recipe, notes_recipe, media_group_recipe = self.parts_from_crowdanki(deck_path)

        build_part_tasks: List[BuildPartTask] = [
            headers_recipe,
            notes_recipe,
            note_models_all_recipe,
            media_group_recipe,
        ]
        dp_builder = PartsBuilder(build_part_tasks)

        top_level_tasks: List[TopLevelBuildTask] = [dp_builder, save_media_to_folder, generate_csvs]
        self.create_yaml_from_top_level(top_level_tasks, os.path.join(LOC_RECIPES, "anki_to_source"))

        # Source to Anki
        note_models_from_yaml = [
            NoteModelsFromYamlPart.from_repr(NoteModelsFromYamlPart.Representation(name, file))
            for name, file in model_name_to_file_dict.items()
        ]

        media_group_from_folder = MediaGroupFromFolder.from_repr(MediaGroupFromFolder.Representation(
            part_id=RECIPE_MEDIA, source=LOC_MEDIA, recursive=True
        ))

        headers_from_yaml = HeadersFromYamlPart.from_repr(HeadersFromYamlPart.Representation(
            part_id=RECIPE_HEADERS, file=headers_name
        ))

        notes_from_csv = NotesFromCsvs.from_repr({
            'part_id': RECIPE_NOTES,
            'note_model_mappings': note_model_mappings,
            'file_mappings': file_mappings
        })

        build_part_tasks: List[BuildPartTask] = note_models_from_yaml + [
            headers_from_yaml,
            notes_from_csv,
            media_group_from_folder,
        ]
        dp_builder = PartsBuilder(build_part_tasks)

        generate_guids_in_csv = GenerateGuidsInCsvs.from_repr(GenerateGuidsInCsvs.Representation(
            source=csv_files, columns=["guid"]
        ))

        generate_crowdanki = CrowdAnkiGenerate.from_repr(CrowdAnkiGenerate.Representation(
            folder=deck_path,
            notes=NotesToCrowdAnki.Representation(
                part_id=RECIPE_NOTES
            ).encode(),
            headers=RECIPE_HEADERS,
            media=MediaGroupToCrowdAnki.Representation(
                parts=[RECIPE_MEDIA]
            ).encode(),
            note_models=NoteModelsToCrowdAnki.Representation(
                parts=[NoteModelsToCrowdAnki.NoteModelListItem.Representation(name).encode()
                       for name, file in model_name_to_file_dict.items()]
            ).encode()
        ))

        top_level_tasks: List[TopLevelBuildTask] = [generate_guids_in_csv, dp_builder, generate_crowdanki]
        source_to_anki_path = os.path.join(LOC_RECIPES, "source_to_anki")
        self.create_yaml_from_top_level(top_level_tasks, source_to_anki_path)

        print(f"\nRepo Init complete. You should now run `brainbrew run {source_to_anki_path}`")

    @staticmethod
    def create_yaml_from_top_level(top_tasks: List[TopLevelBuildTask], filepath: str):
        tl_builder = TopLevelBuilder(top_tasks)

        encoded_top_level_tasks = tl_builder.encode()
        # print(encoded_top_level_tasks)

        model_yaml_file_name = YamlObject.to_filename_yaml(filepath)
        YamlObject.dump_to_yaml_file(model_yaml_file_name, encoded_top_level_tasks)

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

    @staticmethod
    def setup_repo_structure():
        create_path_if_not_exists(LOC_RECIPES)
        create_path_if_not_exists(LOC_BUILD)
        create_path_if_not_exists(LOC_DATA)
        create_path_if_not_exists(LOC_HEADERS)
        create_path_if_not_exists(LOC_NOTE_MODELS)
        create_path_if_not_exists(LOC_MEDIA)
