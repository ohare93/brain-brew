from dataclasses import dataclass

from brain_brew.representation.build_config.build_tasks import add_build_task
from brain_brew.representation.deck_part_transformers.tr_notes_csv_collection import TrNotesToCsvCollection


@dataclass
class GenerateCsvCollection:
    @dataclass
    class Representation:
        file: str
        notes: dict

    file: str
    notes: TrNotesToCsvCollection


add_build_task("Generate Csv Collection", GenerateCsvCollection)
