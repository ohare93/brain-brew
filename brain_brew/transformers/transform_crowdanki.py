from dataclasses import dataclass
from typing import List, Optional, Union

from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiNoteWrapper
from brain_brew.representation.yaml.note_repr import Note
from brain_brew.transformers.base_transform_notes import TrNotes
from brain_brew.utils import blank_str_if_none

from brain_brew.representation.json.wrappers_for_crowd_anki import CA_NOTE_MODELS, CA_NOTES, CA_MEDIA_FILES,\
    CA_CHILDREN, CA_TYPE


class TransformCrowdAnki(TrNotes):
    headers_skip_keys = [CA_NOTE_MODELS, CA_NOTES, CA_MEDIA_FILES]
    headers_default_values = {
        CA_TYPE: "Deck",
        CA_CHILDREN: [],
    }

    @classmethod
    def notes_to_crowd_anki(cls, notes: List[Note], nm_name_to_id: dict, additional_items_to_add: dict) -> List[dict]:
        resolved_notes: List[dict] = []
        wrapper: CrowdAnkiNoteWrapper = CrowdAnkiNoteWrapper()
        for note in notes:
            current = {
                "__type__": "Note",
                "data": None
            }
            wrapper.data = current

            for key, value in additional_items_to_add.items():
                current[key] = blank_str_if_none(value)

            wrapper.guid = note.guid
            wrapper.fields = note.fields
            wrapper.tags = note.tags
            wrapper.note_model = nm_name_to_id[note.note_model]
            wrapper.flags = note.flags

            resolved_notes.append(current)

        return resolved_notes

    @classmethod
    def crowd_anki_to_notes(cls, notes_json: list, nm_id_to_name: dict) -> List[Note]:
        resolved_notes: List[Note] = []
        wrapper: CrowdAnkiNoteWrapper = CrowdAnkiNoteWrapper()
        for note in notes_json:
            wrapper.data = note

            resolved_notes.append(Note(
                note_model=nm_id_to_name[wrapper.note_model],
                tags=wrapper.tags,
                guid=wrapper.guid,
                fields=wrapper.fields,
                flags=wrapper.flags
            ))
        return resolved_notes

    @classmethod
    def headers_to_crowd_anki(cls, headers_data: dict):
        return {**headers_data, **cls.headers_default_values}

    @classmethod
    def crowd_anki_to_headers(cls, ca_data: dict):
        return {key: value for key, value in ca_data
                if key not in cls.headers_skip_keys and key not in cls.headers_default_values.keys()}
