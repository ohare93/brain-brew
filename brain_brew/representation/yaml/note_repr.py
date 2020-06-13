from brain_brew.representation.yaml.my_yaml import yaml_dump, yaml_load
import json
from dataclasses import dataclass
from typing import List, Optional

FIELDS = 'fields'
GUID = 'guid'
TAGS = 'tags'
NOTE_MODEL = 'note_model'


@dataclass
class OverwritableNoteData:
    note_model: Optional[str]
    tags: Optional[List[str]]

    def encode_overwritable(self, json_dict):
        if self.note_model is not None:
            json_dict.setdefault(NOTE_MODEL, self.note_model)
        if self.tags is not None and self.tags != []:
            json_dict.setdefault(TAGS, self.tags)
        return json_dict


@dataclass
class Note(OverwritableNoteData):
    fields: List[str]
    guid: str

    @classmethod
    def from_dict(cls, data: dict):
        return cls(
            fields=data.get(FIELDS),
            guid=data.get(GUID),
            note_model=data.get(NOTE_MODEL, None),
            tags=data.get(TAGS, None)
        )

    def encode(self):
        json_dict = {FIELDS: self.fields, GUID: self.guid}
        super().encode_overwritable(json_dict)
        return json_dict

    def dump_to_yaml(self, file):
        with open(file, 'w') as fp:
            yaml_dump.dump(self.encode(), fp)


@dataclass
class NoteGrouping(OverwritableNoteData):
    notes: List[Note]

    @classmethod
    def from_dict(cls, data):
        return cls(
            notes=list(map(Note.from_dict, data.get("notes"))),
            note_model=data.get(NOTE_MODEL, None),
            tags=data.get(TAGS, None)
        )

    def encode(self):
        json_dict = {}
        super().encode_overwritable(json_dict)
        json_dict.setdefault("notes", [note.encode() for note in self.notes])
        return json_dict

    def dump_to_yaml(self, file):
        with open(file, 'w') as fp:
            yaml_dump.dump(self.encode(), fp)

# @dataclass
# class DeckPartNoteModel: