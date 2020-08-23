from dataclasses import dataclass
import logging
from typing import Union, Optional, List

from brain_brew.build_tasks.crowd_anki.headers_from_crowdanki import HeadersFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.headers_to_crowd_anki import HeadersToCrowdAnki
from brain_brew.build_tasks.crowd_anki.media_to_from_crowd_anki import MediaToFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.note_models_from_crowd_anki import NoteModelsFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.note_models_to_crowd_anki import NoteModelsToCrowdAnki
from brain_brew.build_tasks.crowd_anki.notes_from_crowd_anki import NotesFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.notes_to_crowd_anki import NotesToCrowdAnki
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiJsonWrapper
from brain_brew.representation.yaml.note_model_repr import NoteModel

from brain_brew.representation.build_config.build_task import TopLevelBuildTask
from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.utils import all_combos_prepend_append


@dataclass
class CrowdAnkiGenerate(TopLevelBuildTask):
    task_names = all_combos_prepend_append(["CrowdAnki", "CrowdAnki Export"], "Generate ", "s")

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
            notes_transform=NotesToCrowdAnki.from_repr(rep.notes),
            note_model_transform=NoteModelsToCrowdAnki.from_list(rep.note_models),
            headers_transform=HeadersToCrowdAnki.from_repr(rep.headers),
            media_transform=MediaToFromCrowdAnki.from_repr(rep.media)
        )

    crowd_anki_export: CrowdAnkiExport
    notes_transform: NotesToCrowdAnki
    note_model_transform: NoteModelsToCrowdAnki
    headers_transform: HeadersToCrowdAnki
    media_transform: MediaToFromCrowdAnki

    def execute(self):
        headers = self.headers_transform.execute()
        ca_wrapper = CrowdAnkiJsonWrapper(headers)

        note_models: List[dict] = self.note_model_transform.execute()

        nm_name_to_id: dict = {model.name: model.crowdanki_id for model in self.note_model_transform.note_models}
        notes = self.notes_transform.execute(nm_name_to_id)

        media_files = self.media_transform.move_to_crowd_anki(
            self.notes_transform.notes, self.note_model_transform.note_models, self.crowd_anki_export)

        ca_wrapper.note_models = note_models
        ca_wrapper.notes = notes
        ca_wrapper.media_files = list(media_files)

        #Set to CrowdAnkiExport