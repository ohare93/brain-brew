from dataclasses import dataclass, field
from typing import Union, List, Optional

from brain_brew.representation.build_config.build_task import DeckPartBuildTask
from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.headers_repr import Headers
from brain_brew.representation.yaml.note_model_repr import NoteModel
from brain_brew.representation.yaml.note_repr import Notes


@dataclass
class FromDeckParts(DeckPartBuildTask):
    task_regex = r'from_deck_parts'

    @dataclass
    class DeckPartToRead(RepresentationBase):
        part_id: str
        file: str

    @dataclass
    class Representation(RepresentationBase):
        notes: List[dict] = field(default_factory=list)
        note_models: List[dict] = field(default_factory=list)
        headers: List[dict] = field(default_factory=list)

    notes: List[Notes]
    note_models: List[NoteModel]
    headers: List[Headers]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)

        notes: List[cls.DeckPartToRead] = list(map(FromDeckParts.DeckPartToRead.from_dict, rep.notes))
        note_models: List[cls.DeckPartToRead] = list(map(FromDeckParts.DeckPartToRead.from_dict, rep.note_models))
        headers: List[cls.DeckPartToRead] = list(map(FromDeckParts.DeckPartToRead.from_dict, rep.headers))

        return cls(
            notes=[DeckPartHolder.override_or_create(
                part_id=note.part_id, save_to_file=None, deck_part=Notes.from_file(note.file)) for note in notes],
            note_models=[DeckPartHolder.override_or_create(
                part_id=model.part_id, save_to_file=None, deck_part=NoteModel.from_file(model.file)) for model in note_models],
            headers=[DeckPartHolder.override_or_create(
                part_id=header.part_id, save_to_file=None, deck_part=Headers.from_file(header.file)) for header in headers]
        )

    def execute(self):
        pass
