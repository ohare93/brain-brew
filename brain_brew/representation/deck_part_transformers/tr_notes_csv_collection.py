from dataclasses import dataclass
from typing import List

from brain_brew.representation.configuration.csv_file_mapping import CsvFileMapping
from brain_brew.representation.configuration.note_model_mapping import NoteModelMappingRepresentation
from brain_brew.representation.deck_part_transformers.tr_notes_generic import TrNotesGeneric


@dataclass
class TrNotesCsvCollection(TrNotesGeneric):
    file_mappings: List[CsvFileMapping]
    note_model_mappings: List[NoteModelMappingRepresentation]
