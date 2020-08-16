from dataclasses import dataclass
from typing import Optional

from brain_brew.representation.build_config.representation_base import RepresentationBase


@dataclass
class DeckPartFromBase:
    @dataclass
    class Representation(RepresentationBase):
        name: str
        save_to_file: Optional[str]

        def __init__(self, name, save_to_file=None):
            self.name = name
            self.save_to_file = save_to_file

    name: str
    save_to_file: Optional[str]
