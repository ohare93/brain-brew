from dataclasses import dataclass
from typing import List, Dict

from brain_brew.representation.configuration.csv_file_mapping import CsvFileMapping
from brain_brew.representation.configuration.note_model_mapping import NoteModelMapping
from brain_brew.representation.deck_part_transformers.tr_notes_generic import TrNotesToGeneric, TrGenericToNotes
from brain_brew.representation.yaml.note_repr import DeckPartNotes, Note


@dataclass
class TrCsvCollectionShared:
    @dataclass(init=False)
    class Representation:
        file_mappings: List[CsvFileMapping.Representation]
        note_model_mappings: List[NoteModelMapping.Representation]

        def __init__(self, file_mappings, note_model_mappings):
            self.file_mappings = list(map(CsvFileMapping.Representation.from_dict, file_mappings))
            self.note_model_mappings = list(map(NoteModelMapping.Representation.from_dict, note_model_mappings))

        def get_file_mappings(self) -> List[CsvFileMapping]:
            return list(map(CsvFileMapping.from_repr, self.file_mappings))

        def get_note_model_mappings(self) -> Dict[str, NoteModelMapping]:
            note_model_mappings: Dict[str, NoteModelMapping] = {}
            for nmm in self.note_model_mappings:
                if
            return dict(map(lambda nmm: (nmm.note_model, NoteModelMapping.from_repr(nmm)), ))

    file_mappings: List[CsvFileMapping]
    note_model_mappings: Dict[str, NoteModelMapping]


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
            file_mappings=data.get_file_mappings(),
            note_model_mappings=data.get_note_model_mappings()
        )

    @classmethod
    def from_dict(cls, data: dict):
        return cls.from_repr(TrCsvCollectionToNotes.Representation.from_dict(data))

    def notes_to_deck_parts(self):
        csv_data_by_guid: Dict[str, dict] = {}
        for csv_map in self.file_mappings:
            csv_map.compile_data()
            csv_data_by_guid = {**csv_data_by_guid, **csv_map.compiled_data}
        csv_rows: List[dict] = list(csv_data_by_guid.values())

        notes_json: List[Note] = []

        # Get Guid, Tags, NoteTypeName, Fields
        for row in csv_rows:
            row_nm: NoteModelMapping = self.note_model_mappings_dict[row[DeckPartNoteKeys.NOTE_MODEL.value]]

            filtered_fields = row_nm.csv_row_map_to_note_fields(row)

            note_model = row_nm.note_model.name
            guid = filtered_fields.pop(DeckPartNoteKeys.GUID.value)
            tags = self.split_tags(filtered_fields.pop(DeckPartNoteKeys.TAGS.value))

            fields = row_nm.field_values_in_note_model_order(note_model, filtered_fields)

            notes_json.append(Note(guid=guid, tags=tags, note_model=note_model, fields=fields))

        return notes_json


@dataclass
class TrNotesToCsvCollection(TrCsvCollectionShared, TrNotesToGeneric):
    @dataclass(init=False)
    class Representation(TrCsvCollectionShared.Representation, TrNotesToGeneric.Representation):
        def __init__(self, name, file_mappings, note_model_mappings):
            TrCsvCollectionShared.Representation.__init__(self, file_mappings, note_model_mappings)
            TrNotesToGeneric.Representation.__init__(self, name)

        @classmethod
        def from_dict(cls, data: dict):
            return cls(**data)

    @classmethod
    def from_repr(cls, data: Representation):
        return cls(
            notes=DeckPartNotes.create(data.name, read_now=True),  #TODO: remove old DeckPartNotes. Use pool of DeckParts
            file_mappings=data.get_file_mappings(),
            note_model_mappings=data.get_note_model_mappings()
        )

    @classmethod
    def from_dict(cls, data: dict):
        return cls.from_repr(TrCsvCollectionToNotes.Representation.from_dict(data))
