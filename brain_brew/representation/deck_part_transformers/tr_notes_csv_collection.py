from dataclasses import dataclass
from typing import List

from brain_brew.representation.configuration.csv_file_mapping import CsvFileMapping
from brain_brew.representation.configuration.note_model_mapping import NoteModelMapping
from brain_brew.representation.deck_part_transformers.tr_notes_generic import TrNotesGeneric


@dataclass
class TrNotesCsvCollection(TrNotesGeneric):
    @dataclass(init=False)
    class Representation(TrNotesGeneric.Representation):
        file_mappings: List[CsvFileMapping.Representation]
        note_model_mappings: List[NoteModelMapping.Representation]

        def __init__(self, name, file_mappings, note_model_mappings, save_to_file=None):
            super().__init__(name, save_to_file)
            self.file_mappings = list(map(CsvFileMapping.Representation.from_dict, file_mappings))
            self.note_model_mappings = list(map(NoteModelMapping.Representation.from_dict, note_model_mappings))

        @classmethod
        def from_dict(cls, data: dict):
            return cls(**data)

    file_mappings: List[CsvFileMapping]
    note_model_mappings: List[NoteModelMapping]

    @classmethod
    def from_repr(cls, data: Representation):
        return cls(
            name=data.name,
            save_to_file=data.save_to_file,
            file_mappings=list(map(CsvFileMapping.from_repr, data.file_mappings)),
            note_model_mappings=list(map(NoteModelMapping.from_repr, data.note_model_mappings))
        )

    @classmethod
    def from_dict(cls, data: dict):
        return cls.from_repr(TrNotesCsvCollection.Representation.from_dict(data))


# TODO: Make Unique classes for Notes <-> Csv
