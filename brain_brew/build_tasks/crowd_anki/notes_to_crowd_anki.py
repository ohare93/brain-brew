from dataclasses import dataclass
from typing import Optional, Union, List

from brain_brew.build_tasks.crowd_anki.shared_base_notes import SharedBaseNotes
from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiNoteWrapper
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.note_repr import Notes, Note
from brain_brew.utils import blank_str_if_none


@dataclass
class NotesToCrowdAnki(SharedBaseNotes):
    @dataclass
    class Representation(SharedBaseNotes.Representation):
        deck_part: str
        additional_items_to_add: Optional[dict]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            notes=DeckPartHolder.from_deck_part_pool(rep.deck_part),
            sort_order=SharedBaseNotes._get_sort_order(rep.sort_order),
            additional_items_to_add=rep.additional_items_to_add or {}
        )

    notes: Notes
    additional_items_to_add: dict

    def execute(self, nm_name_to_id: dict) -> List[dict]:
        notes = self.notes.get_notes()

        note_dicts = [self.note_to_ca_note(note, nm_name_to_id, self.additional_items_to_add) for note in notes]

        # TODO: Sort

        return note_dicts

    @staticmethod
    def note_to_ca_note(note: Note, nm_name_to_id: dict, additional_items_to_add: dict) -> dict:
        wrapper = CrowdAnkiNoteWrapper({
            "__type__": "Note",
            "data": None
        })

        for key, value in additional_items_to_add.items():
            wrapper.data[key] = blank_str_if_none(value)

        wrapper.guid = note.guid
        wrapper.fields = note.fields
        wrapper.tags = note.tags
        wrapper.note_model = nm_name_to_id[note.note_model]
        wrapper.flags = note.flags

        return wrapper.data
