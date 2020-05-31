import json
from typing import List

import pytest

from brain_brew.representation.json_refactor.note_repr import Note, NoteGrouping


def create_note_json(fields, guid, note_model, tags):
    json_data = {
        "fields": fields,
        "guid": guid
    }

    if tags is not None and tags != []:
        json_data.setdefault("tags", tags)
    if note_model is not None:
        json_data.setdefault("note_model", note_model)

    return json_data


def create_note_grouping_json(notes, note_model, tags):
    json_data = {"notes": notes}

    if tags is not None and tags != []:
        json_data.setdefault("tags", tags)
    if note_model is not None:
        json_data.setdefault("note_model", note_model)

    return json_data


@pytest.fixture()
def note_test1():
    return Note(note_model="model", tags=['noun', 'other'], fields=['first'], guid="12345")


@pytest.mark.parametrize("fields, guid, note_model, tags", [
    ([], "", "", []),
    (None, None, None, None),
    (["test", "blah", "whatever"], "1234567890", "model_name", ["noun"])
])
class TestNote:
    def test_constructor(self, fields: List[str], guid: str, note_model: str, tags: List[str]):
        note = Note(fields=fields, guid=guid, note_model=note_model, tags=tags)

        assert isinstance(note, Note)
        assert note.fields == fields
        assert note.guid == guid
        assert note.note_model == note_model
        assert note.tags == tags

    def test_from_json(self, fields: List[str], guid: str, note_model: str, tags: List[str]):
        json_data = create_note_json(fields, guid, note_model, tags)
        note = Note.from_json(json_data)

        assert isinstance(note, Note)
        assert note.fields == fields
        assert note.guid == guid
        assert note.note_model == note_model
        assert note.tags == tags or (tags == [] and note.tags is None)

    def test_dump_to_json(self, fields: List[str], guid: str, note_model: str, tags: List[str]):
        json_data = create_note_json(fields, guid, note_model, tags)
        note = Note.from_json(json_data)

        assert note.dump_json_to_string() == json.dumps(json_data, sort_keys=False, indent=4, ensure_ascii=False)


@pytest.mark.parametrize("note_model, tags", [
    ("", []),
    (None, None),
    ("model_name", ["noun"])
])
class TestNoteGrouping:
    def test_constructor(self, note_model: str, tags: List[str], note_test1):
        group = NoteGrouping(notes=[note_test1], note_model=note_model, tags=tags)

        assert isinstance(group, NoteGrouping)
        assert group.notes == [note_test1]
        assert group.note_model == note_model
        assert group.tags == tags

    def test_from_json(self, note_model: str, tags: List[str], note_test1):
        json_data = create_note_grouping_json([Note.encode(note_test1)], note_model, tags)
        group = NoteGrouping.from_json(json_data)

        assert isinstance(group, NoteGrouping)
        assert group.notes == [note_test1]
        assert group.note_model == note_model
        assert group.tags == tags or (tags == [] and group.tags is None)

    def test_dump_to_json(self, note_model: str, tags: List[str], note_test1):
        json_data = create_note_grouping_json([Note.encode(note_test1)], note_model, tags)
        group = NoteGrouping.from_json(json_data)

        assert group.dump_json_to_string() == json.dumps(json_data, sort_keys=False, indent=4, ensure_ascii=False)