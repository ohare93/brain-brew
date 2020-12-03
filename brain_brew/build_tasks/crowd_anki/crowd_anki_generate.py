from dataclasses import dataclass, field
from typing import Union, Optional, List, Set

from brain_brew.build_tasks.crowd_anki.headers_to_crowd_anki import HeadersToCrowdAnki
from brain_brew.build_tasks.crowd_anki.media_to_crowd_anki import MediaGroupToCrowdAnki
from brain_brew.build_tasks.crowd_anki.note_models_to_crowd_anki import NoteModelsToCrowdAnki
from brain_brew.build_tasks.crowd_anki.notes_to_crowd_anki import NotesToCrowdAnki
from brain_brew.configuration.build_config.build_task import TopLevelBuildTask
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.representation.generic.media_file import MediaFile
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiJsonWrapper


@dataclass
class CrowdAnkiGenerate(TopLevelBuildTask):
    @classmethod
    def task_name(cls) -> str:
        return r'generate_crowd_anki'

    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            folder: str()
            headers: str()
            notes: include('{NotesToCrowdAnki.task_name()}')
            note_models: include('{NoteModelsToCrowdAnki.task_name()}')
            media: include('{MediaGroupToCrowdAnki.task_name()}', required=False)
        '''

    @classmethod
    def yamale_dependencies(cls) -> set:
        return {NotesToCrowdAnki, NoteModelsToCrowdAnki, MediaGroupToCrowdAnki}

    @dataclass
    class Representation(RepresentationBase):
        folder: str
        notes: dict
        note_models: dict
        headers: dict
        media: Optional[dict] = field(default_factory=lambda: dict())

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            crowd_anki_export=CrowdAnkiExport.create_or_get(rep.folder),
            notes_transform=NotesToCrowdAnki.from_repr(rep.notes),
            note_model_transform=NoteModelsToCrowdAnki.from_repr(rep.note_models),
            headers_transform=HeadersToCrowdAnki.from_repr(rep.headers),
            media_transform=MediaGroupToCrowdAnki.from_repr(rep.media) if rep.media else None
        )

    crowd_anki_export: CrowdAnkiExport
    notes_transform: NotesToCrowdAnki
    note_model_transform: NoteModelsToCrowdAnki
    headers_transform: HeadersToCrowdAnki
    media_transform: Optional[MediaGroupToCrowdAnki]

    def execute(self):
        headers = self.headers_transform.execute()
        ca_wrapper = CrowdAnkiJsonWrapper(headers)

        note_models: List[dict] = self.note_model_transform.execute()

        nm_name_to_id: dict = {model.part_id: model.part.id for model in self.note_model_transform.note_models}
        notes = self.notes_transform.execute(nm_name_to_id)

        media_files: Set[MediaFile] = set()
        if self.media_transform:
            media_files = self.media_transform.execute(self.crowd_anki_export.media_loc)

        ca_wrapper.media_files = sorted([m.filename for m in media_files])
        ca_wrapper.name = self.headers_transform.headers.name
        ca_wrapper.note_models = note_models
        ca_wrapper.notes = notes

        # Set to CrowdAnkiExport
        self.crowd_anki_export.write_to_files(ca_wrapper.data)
