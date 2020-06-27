from dataclasses import dataclass
from typing import Optional

from brain_brew.representation.yaml.note_repr import DeckPartNotes


@dataclass
class TrNotesGeneric:
    name: str
    data: DeckPartNotes
    save_to_file: Optional[str]
