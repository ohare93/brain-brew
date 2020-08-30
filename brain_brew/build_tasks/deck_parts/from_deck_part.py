from dataclasses import dataclass, field
from typing import Union, List, Optional

from brain_brew.representation.build_config.build_task import DeckPartBuildTask
from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.headers_repr import Headers
from brain_brew.representation.yaml.note_model_repr import NoteModel
from brain_brew.representation.yaml.note_repr import Notes
from brain_brew.utils import all_combos_prepend_append


@dataclass
class FromDeckParts(DeckPartBuildTask):
    task_names = all_combos_prepend_append(["DeckPart", "ReadDeckPart"], "From ", "s")

    @dataclass
    class DeckPartToRead(RepresentationBase):
        name: str
        file: str

    @dataclass
    class Representation(RepresentationBase):
        notes: List[dict] = field(default_factory=list)
        note_models: List[dict] = field(default_factory=list)
        headers: List[dict] = field(default_factory=list)

    notes: List[DeckPartToRead]
    note_models: List[DeckPartToRead]
    headers: List[DeckPartToRead]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            notes=list(map(FromDeckParts.DeckPartToRead.from_dict, rep.notes)),
            note_models=list(map(FromDeckParts.DeckPartToRead.from_dict, rep.note_models)),
            headers=list(map(FromDeckParts.DeckPartToRead.from_dict, rep.headers))
        )

    def execute(self):
        for note in self.notes:
            DeckPartHolder.override_or_create(name=note.name, save_to_file=None, deck_part=Notes.from_file(note.file))
        for model in self.note_models:
            DeckPartHolder.override_or_create(name=model.name, save_to_file=None, deck_part=NoteModel.from_file(model.file))
        for header in self.headers:
            DeckPartHolder.override_or_create(name=header.name, save_to_file=None, deck_part=Headers.from_file(header.file))
