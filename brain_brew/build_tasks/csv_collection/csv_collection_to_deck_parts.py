from dataclasses import dataclass

from brain_brew.build_tasks.csv_collection.config.csv_collection_to_notes import CsvCollectionToNotes
from brain_brew.representation.build_config.build_task import DeckPartBuildTask
from brain_brew.utils import all_combos_prepend_append


@dataclass
class CsvCollectionToDeckParts(DeckPartBuildTask):
    task_names = all_combos_prepend_append(["Csv Collection", "Csv"], "From ", "s")

    @dataclass
    class Representation:
        notes: dict

        @classmethod
        def from_dict(cls, data: dict):
            return cls(**data)

    notes_transform: CsvCollectionToNotes

    @classmethod
    def from_repr(cls, data: Representation):
        return cls(
            notes_transform=CsvCollectionToNotes.from_dict(data.notes)
        )

    @classmethod
    def from_dict(cls, data: dict):
        return cls.from_repr(cls.Representation.from_dict(data))

    def execute(self):
        self.notes_transform.execute()
