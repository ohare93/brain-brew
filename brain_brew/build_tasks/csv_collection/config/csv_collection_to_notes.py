from dataclasses import dataclass
from typing import Dict, List

from brain_brew.build_tasks.csv_collection.config.csv_collection_shared import CsvCollectionShared
from brain_brew.representation.transformers.generic_to_deck_part import TrGenericToDeckPart
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.note_repr import Note, Notes
from brain_brew.transformers.transform_csv_collection import TransformCsvCollection


@dataclass
class CsvCollectionToNotes(CsvCollectionShared, TrGenericToDeckPart):
    @dataclass(init=False)
    class Representation(CsvCollectionShared.Representation, TrGenericToDeckPart.Representation):
        def __init__(self, name, file_mappings, note_model_mappings, save_to_file=None):
            CsvCollectionShared.Representation.__init__(self, file_mappings, note_model_mappings)
            TrGenericToDeckPart.Representation.__init__(self, name, save_to_file)

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
        return cls.from_repr(cls.Representation.from_dict(data))

    def execute(self):
        csv_data_by_guid: Dict[str, dict] = {}
        for csv_map in self.file_mappings:
            csv_map.compile_data()
            csv_data_by_guid = {**csv_data_by_guid, **csv_map.compiled_data}
        csv_rows: List[dict] = list(csv_data_by_guid.values())

        deck_part_notes: List[Note] = TransformCsvCollection.csv_collection_to_notes(
            csv_rows, self.note_model_mappings)

        notes = Notes.from_list_of_notes(deck_part_notes)
        DeckPartHolder.override_or_create(self.name, self.save_to_file, notes)