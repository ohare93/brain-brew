from dataclasses import dataclass, field
from typing import Union, Optional, List

from brain_brew.build_tasks.crowd_anki.shared_base_notes import SharedBaseNotes
from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiJsonWrapper, CrowdAnkiNoteWrapper
from brain_brew.representation.transformers.base_deck_part_from import BaseDeckPartsFrom
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.note_repr import Notes, Note


@dataclass
class NotesFromCrowdAnki(SharedBaseNotes, BaseDeckPartsFrom):
    @dataclass
    class Representation(BaseDeckPartsFrom.Representation):
        sort_order: Optional[List[str]] = field(default_factory=lambda: None)
        reverse_sort: Optional[bool] = field(default_factory=lambda: None)

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            name=rep.name,
            sort_order=SharedBaseNotes._get_sort_order(rep.sort_order),
            reverse_sort=SharedBaseNotes._get_reverse_sort(rep.reverse_sort),
            save_to_file=rep.save_to_file
        )

    sort_order: Optional[List[str]] = field(default_factory=lambda: None)
    reverse_sort: Optional[bool] = field(default_factory=lambda: None)

    def execute(self, ca_wrapper: CrowdAnkiJsonWrapper, nm_id_to_name: dict) -> Notes:
        note_list = [self.ca_note_to_note(note, nm_id_to_name) for note in ca_wrapper.notes]

        notes = Notes.from_list_of_notes(note_list)  # TODO: pass in sort method

        DeckPartHolder.override_or_create(self.name, self.save_to_file, notes)

        return notes

    @staticmethod
    def ca_note_to_note(note: dict, nm_id_to_name: dict) -> Note:
        wrapper = CrowdAnkiNoteWrapper(note)

        return Note(
            note_model=nm_id_to_name[wrapper.note_model],
            tags=wrapper.tags,
            guid=wrapper.guid,
            fields=wrapper.fields,
            flags=wrapper.flags
        )
