import logging
from dataclasses import dataclass
from typing import Union, Optional, List

from brain_brew.build_tasks.crowd_anki.headers_from_crowdanki import HeadersFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.media_to_from_crowd_anki import MediaToFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.note_models_from_crowd_anki import NoteModelsFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.notes_from_crowd_anki import NotesFromCrowdAnki
from brain_brew.representation.build_config.build_task import DeckPartBuildTask
from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.representation.yaml.note_model_repr import NoteModel
from brain_brew.utils import all_combos_prepend_append


@dataclass
class CrowdAnkiToDeckParts(DeckPartBuildTask):
    task_regex = r'.*crowd[\s_-]*?anki.*'

    @dataclass
    class Representation(RepresentationBase):
        folder: str
        notes: Optional[dict]
        note_models: Optional[list]
        headers: Optional[dict]
        media: Optional[dict]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            crowd_anki_export=CrowdAnkiExport.create_or_get(rep.folder),
            notes_transform=NotesFromCrowdAnki.from_repr(rep.notes),
            note_model_transform=NoteModelsFromCrowdAnki.from_list(rep.note_models),
            headers_transform=HeadersFromCrowdAnki.from_repr(rep.headers),
            media_transform=MediaToFromCrowdAnki.from_repr(rep.media)
        )

    crowd_anki_export: CrowdAnkiExport
    notes_transform: NotesFromCrowdAnki
    note_model_transform: NoteModelsFromCrowdAnki
    headers_transform: HeadersFromCrowdAnki
    media_transform: MediaToFromCrowdAnki

    def execute(self):
        ca_wrapper = self.crowd_anki_export.read_json_file()

        if ca_wrapper.children:
            logging.warning("Child Decks / Sub-decks are not currently supported.")

        note_models: List[NoteModel] = self.note_model_transform.execute(ca_wrapper)

        nm_id_to_name: dict = {model.id: model.name for model in note_models}
        notes = self.notes_transform.execute(ca_wrapper, nm_id_to_name)

        headers = self.headers_transform.execute(ca_wrapper)

        media_files = self.media_transform.move_to_deck_parts(notes, note_models, self.crowd_anki_export)
        for media in media_files:
            media.copy_source_to_target()
