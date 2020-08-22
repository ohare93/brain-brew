from dataclasses import dataclass
from typing import Optional, Union, List

from brain_brew.build_tasks.crowd_anki.shared_base_notes import SharedBaseNotes


@dataclass
class NotesToCrowdAnki(SharedBaseNotes):
    @dataclass
    class Representation(SharedBaseNotes.Representation):
        name: str
        additional_items_to_add: Optional[dict]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            name=rep.name,
            sort_order=SharedBaseNotes._get_sort_order(rep.sort_order),
            additional_items_to_add=rep.additional_items_to_add or {}
        )

    name: str
    additional_items_to_add: dict
