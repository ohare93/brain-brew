from dataclasses import dataclass
from typing import Optional, Union, List

from brain_brew.build_tasks.crowd_anki.shared_base_notes import SharedBaseNotes
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.note_repr import Notes
from brain_brew.transformers.transform_crowdanki import TransformCrowdAnki


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
            notes=DeckPartHolder.from_deck_part_pool(rep.name),
            sort_order=SharedBaseNotes._get_sort_order(rep.sort_order),
            additional_items_to_add=rep.additional_items_to_add or {}
        )

    notes: Notes
    additional_items_to_add: dict

    def execute(self, nm_name_to_id: dict) -> List[dict]:
        notes = self.notes.get_notes()

        note_dicts = TransformCrowdAnki.notes_to_crowd_anki(notes, nm_name_to_id, self.additional_items_to_add)

        # TODO: Sort

        return note_dicts
