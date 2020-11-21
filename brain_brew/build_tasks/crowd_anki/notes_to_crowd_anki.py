from dataclasses import dataclass, field
from typing import Optional, Union, List

from brain_brew.build_tasks.crowd_anki.shared_base_notes import SharedBaseNotes
from brain_brew.interfaces.yamale_verifyable import YamlRepr
from brain_brew.representation.configuration.representation_base import RepresentationBase
from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiNoteWrapper
from brain_brew.representation.yaml.note_repr import Notes, Note
from brain_brew.representation.yaml.part_holder import PartHolder
from brain_brew.utils import blank_str_if_none


@dataclass
class NotesToCrowdAnki(YamlRepr, SharedBaseNotes):
    @classmethod
    def task_regex(cls) -> str:
        return r'notes_to_crowd_anki'

    @classmethod
    def yamale_validator_and_deps(cls) -> (str, set):
        return f'''\
            {cls.task_regex()}:
              part_id: str()
              sort_order: list(str(), required=False)
              reverse_sort: bool(required=False)
              additional_items_to_add: map(str(), key=str(), required=False)
        ''', None

    @dataclass
    class Representation(RepresentationBase):
        part_id: str
        additional_items_to_add: Optional[dict] = field(default_factory=lambda: None)
        sort_order: Optional[List[str]] = field(default_factory=lambda: None)
        reverse_sort: Optional[bool] = field(default_factory=lambda: None)

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            notes_to_read=rep.part_id,
            sort_order=SharedBaseNotes._get_sort_order(rep.sort_order),
            reverse_sort=SharedBaseNotes._get_reverse_sort(rep.reverse_sort),
            additional_items_to_add=rep.additional_items_to_add or {}
        )

    notes: Notes = field(init=False)

    notes_to_read: str
    additional_items_to_add: dict
    sort_order: Optional[List[str]] = field(default_factory=lambda: None)
    reverse_sort: Optional[bool] = field(default_factory=lambda: None)

    def execute(self, nm_name_to_id: dict) -> List[dict]:
        self.notes = PartHolder.from_file_manager(self.notes_to_read).part

        notes = self.notes.get_sorted_notes_copy(sort_by_keys=self.sort_order, reverse_sort=self.reverse_sort)

        note_dicts = [self.note_to_ca_note(note, nm_name_to_id, self.additional_items_to_add) for note in notes]

        return note_dicts

    @staticmethod
    def note_to_ca_note(note: Note, nm_name_to_id: dict, additional_items_to_add: dict) -> dict:
        wrapper = CrowdAnkiNoteWrapper({
            "__type__": "Note",
            "data": ""
        })

        for key, value in additional_items_to_add.items():
            wrapper.data[key] = blank_str_if_none(value)

        wrapper.fields = note.fields
        wrapper.flags = note.flags
        wrapper.guid = note.guid
        wrapper.note_model = nm_name_to_id[note.note_model]
        wrapper.tags = note.tags

        return wrapper.data
