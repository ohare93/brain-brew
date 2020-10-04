from dataclasses import dataclass
from typing import Union

from brain_brew.build_tasks.csvs.notes_from_csvs import NotesFromCsvs
from brain_brew.representation.build_config.build_task import DeckPartBuildTask
from brain_brew.representation.build_config.representation_base import RepresentationBase


@dataclass
class CsvsToDeckParts(DeckPartBuildTask):
    task_regex = r'from_csvs'

    @dataclass
    class Representation(RepresentationBase):
        notes: dict

    notes_transform: NotesFromCsvs

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            notes_transform=NotesFromCsvs.from_repr(rep.notes)
        )

    def execute(self):
        self.notes_transform.execute()
