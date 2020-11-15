import logging
from dataclasses import dataclass, field
from typing import Union, Optional, List

from brain_brew.build_tasks.crowd_anki.shared_base_notes import SharedBaseNotes
from brain_brew.representation.build_config.build_task import DeckPartBuildTask
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiJsonWrapper, CrowdAnkiNoteWrapper
from brain_brew.representation.transformers.base_deck_part_from import BaseDeckPartsFrom
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.note_repr import Notes, Note


@dataclass
class NotesFromCrowdAnki(SharedBaseNotes, BaseDeckPartsFrom, DeckPartBuildTask):
    @classmethod
    def task_regex(cls) -> str:
        return r'notes_from_crowd_anki'

    @classmethod
    def yamale_validator_and_deps(cls) -> (str, set):
        return f'''\
            {cls.task_regex()}:
              source: str()
              part_id: str()
              sort_order: list(str(), required=False)
              save_to_file: str(required=False)
              reverse_sort: str(required=False)
        ''', None

    @dataclass
    class Representation(BaseDeckPartsFrom.Representation):
        source: str
        sort_order: Optional[List[str]] = field(default_factory=lambda: None)
        reverse_sort: Optional[bool] = field(default_factory=lambda: None)

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            ca_export=CrowdAnkiExport.create_or_get(rep.source),
            part_id=rep.part_id,
            sort_order=SharedBaseNotes._get_sort_order(rep.sort_order),
            reverse_sort=SharedBaseNotes._get_reverse_sort(rep.reverse_sort),
            save_to_file=rep.save_to_file
        )

    ca_export: CrowdAnkiExport
    sort_order: Optional[List[str]] = field(default_factory=lambda: None)
    reverse_sort: Optional[bool] = field(default_factory=lambda: None)

    def execute(self):
        ca_wrapper: CrowdAnkiJsonWrapper = self.ca_export.json_data
        if ca_wrapper.children:
            logging.warning("Child Decks / Sub-decks are not currently supported.")

        nm_id_to_name: dict = {model.id: model.name for model in self.ca_export.note_models}
        note_list = [self.ca_note_to_note(note, nm_id_to_name) for note in ca_wrapper.notes]

        notes = Notes.from_list_of_notes(note_list)  # TODO: pass in sort method
        DeckPartHolder.override_or_create(self.part_id, self.save_to_file, notes)

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
