from dataclasses import dataclass

from brain_brew.build_tasks.crowd_anki.headers_from_crowdanki import HeadersFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.note_models_all_from_crowd_anki import NoteModelsAllFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.notes_from_crowd_anki import NotesFromCrowdAnki
from brain_brew.build_tasks.deck_parts.save_note_models_to_folder import SaveNoteModelsToFolder
from brain_brew.interfaces.command import Command
from brain_brew.utils import create_path_if_not_exists

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
        headers_from_crowdanki = HeadersFromCrowdAnki.from_repr(HeadersFromCrowdAnki.Representation(
            source=self.crowdanki_folder, part_id="deck_headers"))
        notes_from_crowdanki = NotesFromCrowdAnki.from_repr(NotesFromCrowdAnki.Representation(
            source=self.crowdanki_folder, part_id="deck_notes"))
        note_models_all_from_crowdanki = NoteModelsAllFromCrowdAnki.from_repr(NoteModelsAllFromCrowdAnki.Representation(
            source=self.crowdanki_folder))

        self.setup_repo_structure()

        headers = headers_from_crowdanki.execute().part
        headers.dump_to_yaml(LOC_HEADERS + "header1.yaml")

        note_models = note_models_all_from_crowdanki.execute()
        save_note_models_to_folder = SaveNoteModelsToFolder(note_models, LOC_NOTE_MODELS, True)
        save_note_models_to_folder.execute()


    def setup_repo_structure(self):
        create_path_if_not_exists(LOC_RECIPES)
        create_path_if_not_exists(LOC_BUILD)
        create_path_if_not_exists(LOC_DATA)
        create_path_if_not_exists(LOC_HEADERS)
        create_path_if_not_exists(LOC_NOTE_MODELS)
        create_path_if_not_exists(LOC_MEDIA)


