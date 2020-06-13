import json
import sys
from textwrap import dedent
from typing import List
from ruamel.yaml import round_trip_dump
from brain_brew.representation.yaml.my_yaml import yaml_dump, yaml_load

import pytest

from brain_brew.representation.yaml.note_repr import Note, NoteGrouping

note_values_that_should_work = {
    "test1": {"fields": ['first'], "guid": "12345", "note_model": "model_name", "tags": ['noun', 'other']},
    "test2": {"fields": ['english', 'german'], "guid": "sdfhfghsvsdv", "note_model": "LL Test", "tags": ['marked']},
    "no_note_model": {"fields": ['first'], "guid": "12345", "tags": ['noun', 'other']},
    "no_tags1": {"fields": ['first'], "guid": "12345", "note_model": "model_name"},
    "no_tags2": {"fields": ['first'], "guid": "12345", "note_model": "model_name", "tags": []}
}

note_groupings_that_should_work = {
    "nothing_grouped": {"notes": [note_values_that_should_work["test1"], note_values_that_should_work["test2"]]},
    "note_model_grouped": {
        "notes": [note_values_that_should_work["no_note_model"], note_values_that_should_work["no_note_model"]],
        "note_model": "model_name"}
}


########### Notes
@pytest.fixture(params=note_values_that_should_work.values())
def note_fixtures(request):
    return Note.from_dict(request.param)

# Note Groupings
@pytest.fixture(params=note_groupings_that_should_work.values())
def note_grouping_fixtures(request):
    return NoteGrouping.from_dict(request.param)


class TestConstructor:
    class TestNote:
        @pytest.mark.parametrize("fields, guid, note_model, tags", [
            ([], "", "", []),
            (None, None, None, None),
            (["test", "blah", "whatever"], "1234567890x", "model_name", ["noun"])
        ])
        def test_constructor(self, fields: List[str], guid: str, note_model: str, tags: List[str]):
            note = Note(fields=fields, guid=guid, note_model=note_model, tags=tags)

            assert isinstance(note, Note)
            assert note.fields == fields
            assert note.guid == guid
            assert note.note_model == note_model
            assert note.tags == tags

        def test_from_dict(self, note_fixtures):
            assert isinstance(note_fixtures, Note)

    class TestNoteGrouping:
        def test_constructor(self):
            note_grouping = NoteGrouping(notes=[Note.from_dict(note_values_that_should_work["test1"])], note_model=None, tags=None)

            assert isinstance(note_grouping, NoteGrouping)
            assert isinstance(note_grouping.notes, List)

        def test_from_dict(self, note_grouping_fixtures):
            assert isinstance(note_grouping_fixtures, NoteGrouping)


class TestDumpToYaml:
    class TestNote:
        @staticmethod
        def _assert_dump_to_yaml(tmpdir, ystring, note_name):
            folder = tmpdir.mkdir("yaml_files")
            file = folder.join("test.yaml")
            file.write("test")

            note = Note.from_dict(note_values_that_should_work[note_name])
            note.dump_to_yaml(str(file))

            assert file.read() == ystring

        def test_all1(self, tmpdir):
            ystring = dedent('''\
            fields:
                - first
            guid: '12345'
            note_model: model_name
            tags:
                - noun
                - other
            ''')

            self._assert_dump_to_yaml(tmpdir, ystring, "test1")

        def test_all2(self, tmpdir):
            ystring = dedent('''\
            fields:
                - english
                - german
            guid: sdfhfghsvsdv
            note_model: LL Test
            tags:
                - marked
            ''')

            self._assert_dump_to_yaml(tmpdir, ystring, "test2")

        def test_no_note_model(self, tmpdir):
            ystring = dedent('''\
            fields:
                - first
            guid: '12345'
            tags:
                - noun
                - other
            ''')

            self._assert_dump_to_yaml(tmpdir, ystring, "no_note_model")

        def test_no_tags(self, tmpdir):
            for num, note in enumerate(["no_tags1", "no_tags2"]):
                ystring = dedent('''\
                fields:
                    - first
                guid: '12345'
                note_model: model_name
                ''')

                self._assert_dump_to_yaml(tmpdir.mkdir(str(num)), ystring, note)

    class TestNoteGrouping:
        @staticmethod
        def _assert_dump_to_yaml(tmpdir, ystring, note_grouping_name):
            folder = tmpdir.mkdir("yaml_files")
            file = folder.join("test.yaml")
            file.write("test")

            note = NoteGrouping.from_dict(note_groupings_that_should_work[note_grouping_name])
            note.dump_to_yaml(str(file))

            assert file.read() == ystring

        def test_nothing_grouped(self, tmpdir):
            ystring = dedent('''\
            notes:
                - fields:
                      - first
                  guid: '12345'
                  note_model: model_name
                  tags:
                      - noun
                      - other
                - fields:
                      - english
                      - german
                  guid: sdfhfghsvsdv
                  note_model: LL Test
                  tags:
                      - marked
            ''')

            self._assert_dump_to_yaml(tmpdir, ystring, "nothing_grouped")

        def test_note_model_grouped(self, tmpdir):
            ystring = dedent('''\
            note_model: model_name
            notes:
                - fields:
                      - first
                  guid: '12345'
                  tags:
                      - noun
                      - other
                - fields:
                      - first
                  guid: '12345'
                  tags:
                      - noun
                      - other
            ''')

            self._assert_dump_to_yaml(tmpdir, ystring, "note_model_grouped")
