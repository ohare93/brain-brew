from dataclasses import dataclass
from typing import Union

from brain_brew.build_tasks.csvs.notes_from_csvs import NotesFromCsvs
from brain_brew.representation.build_config.build_task import DeckPartBuildTask
from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.utils import all_combos_prepend_append


@dataclass
class CsvsToDeckParts(DeckPartBuildTask):
    task_names = all_combos_prepend_append(["Csv Collection", "Csv"], "From ", "s")

    @dataclass
    class Representation(RepresentationBase):
        notes: dict

    notes_transform: NotesFromCsvs

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            notes_transform=NotesFromCsvs.from_dict(rep.notes)
        )

    def execute(self):
        self.notes_transform.execute()
