from brain_brew.representation.yaml.my_yaml import yaml_dump, yaml_load
import json
from dataclasses import dataclass
from typing import List, Optional, Dict

FIELDS = 'fields'
GUID = 'guid'
TAGS = 'tags'
NOTE_MODEL = 'note_model'
NOTES = "notes"
NOTE_GROUPINGS = "note_groupings"


@dataclass
class GroupableNoteData:
    note_model: Optional[str]
    tags: Optional[List[str]]

    def encode_groupable(self, data_dict):
        if self.note_model is not None:
            data_dict.setdefault(NOTE_MODEL, self.note_model)
        if self.tags is not None and self.tags != []:
            data_dict.setdefault(TAGS, self.tags)
        return data_dict


@dataclass
class Note(GroupableNoteData):
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

    def encode(self) -> dict:
        data_dict = {FIELDS: self.fields, GUID: self.guid}
        super().encode_groupable(data_dict)
        return data_dict

    def dump_to_yaml(self, file):
        with open(file, 'w') as fp:
            yaml_dump.dump(self.encode(), fp)


@dataclass
class NoteGrouping(GroupableNoteData):
    notes: List[Note]

    @classmethod
    def from_dict(cls, data: dict):
        return cls(
            notes=list(map(Note.from_dict, data.get(NOTES))),
            note_model=data.get(NOTE_MODEL, None),
            tags=data.get(TAGS, None)
        )

    def encode(self) -> dict:
        data_dict = {}
        super().encode_groupable(data_dict)
        data_dict.setdefault(NOTES, [note.encode() for note in self.notes])
        return data_dict

    def dump_to_yaml(self, file):
        with open(file, 'w') as fp:
            yaml_dump.dump(self.encode(), fp)

    def verify_groupings(self):
        errors = []
        if self.note_model is not None:
            if any([note.note_model for note in self.notes]):
                errors.append(ValueError(f"NoteGrouping for 'note_model' {self.note_model} has notes with 'note_model'."
                                         f" Please remove one of these."))
        return errors

    def get_all_notes(self) -> List[Note]:
        def join_tags(n_tags):
            if self.tags is None and n_tags is None:
                return []
            elif self.tags is None:
                return n_tags
            elif n_tags is None:
                return self.tags
            else:
                return [*n_tags, *self.tags]

        return [Note(
                    note_model=self.note_model if self.note_model is not None else n.note_model,
                    tags=join_tags(n.tags),
                    fields=n.fields,
                    guid=n.guid
               ) for n in self.notes]


@dataclass
class DeckPartNotes:
    note_groupings: List[NoteGrouping]

    @classmethod
    def from_dict(cls, data: dict):
        return cls(
            note_groupings=list(map(NoteGrouping.from_dict, data.get(NOTE_GROUPINGS)))
        )

    def encode(self) -> dict:
        data_dict = {NOTE_GROUPINGS: [note_grouping.encode() for note_grouping in self.note_groupings]}
        return data_dict

    def dump_to_yaml(self, file):
        with open(file, 'w') as fp:
            yaml_dump.dump(self.encode(), fp)


