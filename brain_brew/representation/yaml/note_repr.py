from brain_brew.representation.yaml.my_yaml import yaml
import json
from dataclasses import dataclass
from typing import List


@dataclass
class OverwritableNoteData:
    note_model: str
    tags: List[str]

    def encode_overwritable(self, json_dict):
        if self.tags is not None and self.tags != []:
            json_dict.setdefault("tags", self.tags)
        if self.note_model is not None:
            json_dict.setdefault("note_model", self.note_model)
        return json_dict


@dataclass
class Note(OverwritableNoteData):
    fields: List[str]
    guid: str

    @classmethod
    def from_dict(cls, data: dict):
        return cls(
            fields=data.get("fields"),
            guid=data.get("guid"),
            note_model=data.get("note_model", None),
            tags=data.get("tags", None)
        )

    def encode(self):
        json_dict = {"guid": self.guid, "fields": self.fields}
        super().encode_overwritable(json_dict)
        return json_dict

    def dump_to_yaml(self, file):
        yaml.dump(self.encode(), file)


@dataclass
class NoteGrouping(OverwritableNoteData):
    notes: List[Note]

    @classmethod
    def from_dict(cls, data):
        return cls(
            notes=list(map(Note.from_dict, data.get("notes"))),
            note_model=data.get("note_model", None),
            tags=data.get("tags", None)
        )

    def encode(self):
        json_dict = {"notes": [note.encode() for note in self.notes]}
        super().encode_overwritable(json_dict)
        return json_dict

    def dump_to_yaml(self, file):
        yaml.dump(self.encode(), file)

# @dataclass
# class DeckPartNoteModel: