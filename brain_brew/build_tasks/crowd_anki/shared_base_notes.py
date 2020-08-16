from dataclasses import dataclass
from typing import Optional, Union, List

from brain_brew.representation.build_config.representation_base import RepresentationBase


@dataclass
class SharedBaseNotes:
    @dataclass
    class Representation(RepresentationBase):
        name: str
        sort_order: Optional[Union[str, List[str]]]
        move_media: Optional[bool]
        useless_note_keys: Optional[dict]

    @staticmethod
    def _get_sort_order(sort_order: Optional[Union[str, List[str]]]):
        if isinstance(sort_order, list):
            return sort_order
        elif isinstance(sort_order, str):
            return [sort_order]
        return []

    @staticmethod
    def _get_move_media(move_media: Optional[bool]):
        return move_media or False

    @staticmethod
    def _get_useless_note_keys(useless_note_keys: Optional[dict]):
        default_useless_keys = {
            "__type__": "Note",
            "data": None,
            "flags": 0
        }

        if useless_note_keys is None:
            return default_useless_keys

        return {**useless_note_keys, **default_useless_keys}

    name: str
    sort_order: Optional[List[str]]
    move_media: bool
    useless_note_keys: dict
