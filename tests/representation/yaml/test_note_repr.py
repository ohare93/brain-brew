import json
from typing import List
from ruamel.yaml import round_trip_dump
from brain_brew.representation.yaml.my_yaml import yaml

import pytest

from brain_brew.representation.yaml.note_repr import Note, NoteGrouping


@pytest.fixture()
def note_test1():
    return Note(note_model="model", tags=['noun', 'other'], fields=['first'], guid="12345")


@pytest.mark.parametrize("fields, guid, note_model, tags", [
    ([], "", "", []),
    (None, None, None, None),
    (["test", "blah", "whatever"], "1234567890x", "model_name", ["noun"])
])
class TestNote:
    def test_constructor(self, fields: List[str], guid: str, note_model: str, tags: List[str]):
        note = Note(fields=fields, guid=guid, note_model=note_model, tags=tags)

        assert isinstance(note, Note)
        assert note.fields == fields
        assert note.guid == guid
        assert note.note_model == note_model
        assert note.tags == tags

    def test_encode(self, fields: List[str], guid: str, note_model: str, tags: List[str]):
        note = Note(fields=fields, guid=guid, note_model=note_model, tags=tags)

        encoded_dict = note.encode()

        assert "fields" in encoded_dict
        assert "guid" in encoded_dict
        has_tags = "tags" in encoded_dict
        assert has_tags if tags != [] and tags is not None else not has_tags
        has_note_model = "note_model" in encoded_dict
        assert has_note_model if note_model is not None else not has_note_model


class TestNoteFromYaml:
    def test_dump_to_yaml(self, tmpdir, fields: List[str], guid: str, note_model: str, tags: List[str]):
        folder = tmpdir.mkdir("yaml_files")
        file = folder.join("test.yaml")
        file.write("test")

        yaml_string = """\
        guid: 7ysf7ysd8f8
        fields:
            - test
            - blah
            - another one
        tags:
            - noun
            - english
        note_model: LL Noun
        """

        note = Note(fields=fields, guid=guid, note_model=note_model, tags=tags)
        note.dump_to_yaml(file)
        # rt_dump = round_trip_dump(Note.encode(self))

        assert yaml.load(file.read()) == yaml.load(yaml_string)

