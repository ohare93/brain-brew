from dataclasses import dataclass, field
from typing import Optional

from brain_brew.representation.yaml.note_repr import DeckPartNotes


@dataclass
class TrGenericToNotes:
    @dataclass
    class Representation:
        name: str
        save_to_file: Optional[str]

        def __init__(self, name, save_to_file=None):
            self.name = name
            self.save_to_file = save_to_file

    name: str
    save_to_file: Optional[str]

    data: DeckPartNotes = field(init=False)


@dataclass
class TrNotesToGeneric:
    @dataclass
    class Representation:
        name: str

        def __init__(self, name):
            self.name = name

    notes: DeckPartNotes
