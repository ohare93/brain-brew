from dataclasses import dataclass
from typing import Dict, List, Union

from brain_brew.build_tasks.csvs.shared_base_csvs import SharedBaseCsvs
from brain_brew.representation.transformers.base_deck_part_from import BaseDeckPartsFrom
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.note_repr import Note, Notes
from brain_brew.transformers.transform_csv_collection import TransformCsvCollection


@dataclass
class NotesFromCsvs(SharedBaseCsvs, BaseDeckPartsFrom):
    @dataclass(init=False)
    class Representation(SharedBaseCsvs.Representation, BaseDeckPartsFrom.Representation):
        def __init__(self, name, file_mappings, note_model_mappings, save_to_file=None):
            SharedBaseCsvs.Representation.__init__(self, file_mappings, note_model_mappings)
            BaseDeckPartsFrom.Representation.__init__(self, name, save_to_file)

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            name=rep.name,
            save_to_file=rep.save_to_file,
            file_mappings=rep.get_file_mappings(),
            note_model_mappings=rep.get_note_model_mappings()
        )

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
