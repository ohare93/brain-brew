from dataclasses import dataclass
from typing import Optional, Union, List

from brain_brew.representation.build_config.representation_base import RepresentationBase


@dataclass
class SharedBaseNotes:
    @dataclass
    class Representation(RepresentationBase):
        sort_order: Optional[Union[str, List[str]]]

    @staticmethod
    def _get_sort_order(sort_order: Optional[Union[str, List[str]]]):
        if isinstance(sort_order, list):
            return sort_order
        elif isinstance(sort_order, str):
            return [sort_order]
        return []

    sort_order: Optional[List[str]]
