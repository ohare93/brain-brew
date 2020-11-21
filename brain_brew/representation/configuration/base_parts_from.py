from dataclasses import dataclass
from typing import Optional

from brain_brew.representation.configuration.representation_base import RepresentationBase


@dataclass
class BasePartsFrom:
    @dataclass
    class Representation(RepresentationBase):
        part_id: str
        save_to_file: Optional[str]

        def __init__(self, part_id, save_to_file=None):
            self.part_id = part_id
            self.save_to_file = save_to_file

    part_id: str
    save_to_file: Optional[str]
