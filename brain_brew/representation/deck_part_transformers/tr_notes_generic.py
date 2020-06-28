from dataclasses import dataclass, field
from typing import Optional

from brain_brew.representation.yaml.note_repr import DeckPartNotes


@dataclass
class TrNotesGeneric:
    @dataclass
    class Representation:
        name: str
        save_to_file: Optional[str]

    name: str
    save_to_file: Optional[str]

    data: DeckPartNotes = field(init=False)
