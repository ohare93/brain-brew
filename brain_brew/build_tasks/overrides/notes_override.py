from dataclasses import dataclass
from typing import Optional, Union

from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.interfaces.yamale_verifyable import YamlRepr
from brain_brew.representation.yaml.notes import Note


@dataclass
class NotesOverride(YamlRepr):
    @classmethod
    def task_name(cls) -> str:
        return r"notes_override"

    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            note_model: str(required=False)
        '''

    @dataclass
    class Representation(RepresentationBase):
        note_model: Optional[str]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            rep=rep,
            note_model=rep.note_model
        )

    rep: Representation
    note_model: Optional[str]

    def override(self, note: Note):
        if self.note_model:
            note.note_model = self.note_model

        return note
