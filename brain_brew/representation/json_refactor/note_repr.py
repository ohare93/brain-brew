import json
from dataclasses import dataclass
from typing import List


@dataclass
class OverwritableNoteData:
    note_model: str
    tags: List[str]

    @staticmethod
    def encode_overwritable(obj, json_dict):
        if obj.tags is not None and obj.tags != []:
            json_dict.setdefault("tags", obj.tags)
        if obj.note_model is not None:
            json_dict.setdefault("note_model", obj.note_model)
        return json_dict


@dataclass
class Note(OverwritableNoteData):
    fields: List[str]
    guid: str

    @classmethod
    def from_json(cls, data: dict):
        return cls(
            fields=data.get("fields"),
            guid=data.get("guid"),
            note_model=data.get("note_model", None),
            tags=data.get("tags", None)
        )

    @staticmethod
    def encode(obj):
        if isinstance(obj, Note):
            json_dict = {"fields": obj.fields, "guid": obj.guid}
            super().encode_overwritable(obj, json_dict)
            return json_dict
        raise TypeError("Cannot encode object as Note", obj)

    def dump_json_to_string(self):
        return json.dumps(self, default=Note.encode, sort_keys=False, indent=4, ensure_ascii=False)


@dataclass
class NoteGrouping(OverwritableNoteData):
    notes: List[Note]

    @classmethod
    def from_json(cls, data):
        return cls(
            notes=list(map(Note.from_json, data.get("notes"))),
            note_model=data.get("note_model", None),
            tags=data.get("tags", None)
        )

    @staticmethod
    def encode(obj):
        if isinstance(obj, NoteGrouping):
            json_dict = {"notes": [Note.encode(note) for note in obj.notes]}
            super().encode_overwritable(obj, json_dict)
            return json_dict
        raise TypeError("Cannot encode object as NoteGrouping", obj)

    def dump_json_to_string(self):
        return json.dumps(self, default=NoteGrouping.encode, sort_keys=False, indent=4, ensure_ascii=False)


# @dataclass
# class DeckPartNoteModel: