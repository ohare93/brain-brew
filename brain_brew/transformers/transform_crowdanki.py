from dataclasses import dataclass
from typing import List, Optional, Union

from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiNoteWrapper
from brain_brew.representation.yaml.note_repr import Note
from brain_brew.transformers.base_transform_notes import TrNotes
from brain_brew.utils import blank_str_if_none


class TransformCrowdAnki(TrNotes):
    @classmethod
    def crowd_anki_to_notes(cls, notes_json: dict, note_models_id_name_dict) -> List[Note]:
        resolved_notes: List[Note] = []
        wrapper: CrowdAnkiNoteWrapper = CrowdAnkiNoteWrapper()
        for note in notes_json:
            wrapper.data = note

            resolved_notes.append(Note(
                note_model=note_models_id_name_dict[wrapper.note_model],
                tags=wrapper.tags,
                guid=wrapper.guid,
                fields=wrapper.fields
            ))
        return resolved_notes

    @classmethod
    def notes_to_crowd_anki(cls, notes: List[Note], note_models_id_name_dict, useless_note_keys) -> List[dict]:
        resolved_notes: List[dict] = []
        wrapper: CrowdAnkiNoteWrapper = CrowdAnkiNoteWrapper()
        for note in notes:
            current = {}
            wrapper.data = current

            for key in useless_note_keys:
                current[key] = blank_str_if_none(useless_note_keys[key])

            wrapper.note_model = note_models_id_name_dict[note.note_model]
            resolved_notes.append(current)

        return resolved_notes
