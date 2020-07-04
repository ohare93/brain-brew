from dataclasses import dataclass
from typing import List

from brain_brew.representation.configuration.csv_file_mapping import CsvFileMapping
from brain_brew.representation.configuration.note_model_mapping import NoteModelMapping
from brain_brew.representation.deck_part_transformers.tr_notes_generic import TrNotesToGeneric, TrGenericToNotes


@dataclass
class TrCsvCollectionShared:
    @dataclass(init=False)
    class Representation:
        file_mappings: List[CsvFileMapping.Representation]
        note_model_mappings: List[NoteModelMapping.Representation]

        def __init__(self, file_mappings, note_model_mappings):
            self.file_mappings = list(map(CsvFileMapping.Representation.from_dict, file_mappings))
            self.note_model_mappings = list(map(NoteModelMapping.Representation.from_dict, note_model_mappings))

    file_mappings: List[CsvFileMapping]
    note_model_mappings: List[NoteModelMapping]


@dataclass
class TrCsvCollectionToNotes(TrCsvCollectionShared, TrGenericToNotes):
    @dataclass(init=False)
    class Representation(TrCsvCollectionShared.Representation, TrGenericToNotes.Representation):
        def __init__(self, name, file_mappings, note_model_mappings, save_to_file=None):
            TrCsvCollectionShared.Representation.__init__(self, file_mappings, note_model_mappings)
            TrGenericToNotes.Representation.__init__(self, name, save_to_file)

        @classmethod
        def from_dict(cls, data: dict):
            return cls(**data)

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
        return cls.from_repr(TrCsvCollectionToNotes.Representation.from_dict(data))


# TODO: Make Unique classes for Notes <-> Csv
