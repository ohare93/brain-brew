from dataclasses import dataclass
from typing import List, Optional

from brain_brew.representation.deck_part_transformers.tr_notes_generic import TrNotesGeneric


@dataclass
class TrNotesCrowdAnki(TrNotesGeneric):
    file: str
    sort_order: Optional[List[str]]
    media: Optional[bool]
    useless_note_keys: Optional[dict]

    # crowdanki_file: CrowdAnkiExport ????
