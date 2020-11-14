from abc import ABCMeta
from dataclasses import dataclass
from typing import Optional

from brain_brew.representation.build_config.build_task import DeckPartBuildTask
from brain_brew.representation.build_config.representation_base import RepresentationBase


@dataclass
class BaseDeckPartsFrom(DeckPartBuildTask, metaclass=ABCMeta):
    @dataclass
    class Representation(RepresentationBase):
        part_id: str
        save_to_file: Optional[str]

        def __init__(self, part_id, save_to_file=None):
            self.part_id = part_id
            self.save_to_file = save_to_file

    part_id: str
    save_to_file: Optional[str]
