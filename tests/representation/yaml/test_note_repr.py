import json
import sys
from textwrap import dedent
from typing import List
from ruamel.yaml import round_trip_dump
from brain_brew.representation.yaml.my_yaml import yaml_dump, yaml_load

import pytest

from brain_brew.representation.yaml.note_repr import Note, NoteGrouping, DeckPartNotes, \
    NOTES, NOTE_GROUPINGS, FIELDS, GUID, NOTE_MODEL, TAGS

working_notes = {
    "test1": {FIELDS: ['first'], GUID: "12345", NOTE_MODEL: "model_name", TAGS: ['noun', 'other']},
    "test2": {FIELDS: ['english', 'german'], GUID: "sdfhfghsvsdv", NOTE_MODEL: "LL Test", TAGS: ['marked']},
    "no_note_model": {FIELDS: ['first'], GUID: "12345", TAGS: ['noun', 'other']},
    "no_note_model2": {FIELDS: ['second'], GUID: "67890", TAGS: ['noun', 'other']},
    "no_tags1": {FIELDS: ['first'], GUID: "12345", NOTE_MODEL: "model_name"},
    "no_tags2": {FIELDS: ['first'], GUID: "12345", NOTE_MODEL: "model_name", TAGS: []},
    "no_model_or_tags": {FIELDS: ['first'], GUID: "12345"}
}

working_note_groupings = {
    "nothing_grouped": {NOTES: [working_notes["test1"], working_notes["test2"]]},
    "note_model_grouped": {NOTES: [working_notes["no_note_model"], working_notes["no_note_model2"]], NOTE_MODEL: "model_name"},
    "tags_grouped": {NOTES: [working_notes["no_tags1"], working_notes["no_tags2"]], TAGS: ["noun", "other"]},
    "tags_grouped_as_addition": {NOTES: [working_notes["test1"], working_notes["test2"]], TAGS: ["test", "recent"]},
    "model_and_tags_grouped": {NOTES: [working_notes["no_model_or_tags"], working_notes["no_model_or_tags"]], NOTE_MODEL: "model_name", TAGS: ["noun", "other"]}
}


@pytest.fixture(params=working_notes.values())
def note_fixtures(request):
    return Note.from_dict(request.param)


@pytest.fixture(params=working_note_groupings.values())
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
            note_grouping = NoteGrouping(notes=[Note.from_dict(working_notes["test1"])], note_model=None, tags=None)

            assert isinstance(note_grouping, NoteGrouping)
            assert isinstance(note_grouping.notes, List)

        def test_from_dict(self, note_grouping_fixtures):
            assert isinstance(note_grouping_fixtures, NoteGrouping)

    class TestDeckPartNote:
        def test_constructor(self):
            dpn = DeckPartNotes(note_groupings=[NoteGrouping.from_dict(working_note_groupings["nothing_grouped"])])
            assert isinstance(dpn, DeckPartNotes)

        def test_from_dict(self):
            dpn = DeckPartNotes.from_dict({NOTE_GROUPINGS: [working_note_groupings["nothing_grouped"]]})
            assert isinstance(dpn, DeckPartNotes)


class TestDumpToYaml:
    @staticmethod
    def _make_temp_file(tmpdir):
        folder = tmpdir.mkdir("yaml_files")
        file = folder.join("test.yaml")
        file.write("test")
        return file

    class TestNote:
        @staticmethod
        def _assert_dump_to_yaml(tmpdir, ystring, note_name):
            file = TestDumpToYaml._make_temp_file(tmpdir)

            note = Note.from_dict(working_notes[note_name])
            note.dump_to_yaml(str(file))

            assert file.read() == ystring

        def test_all1(self, tmpdir):
            ystring = dedent(f'''\
            {FIELDS}:
              - first
            {GUID}: '12345'
            {NOTE_MODEL}: model_name
            {TAGS}:
              - noun
              - other
            ''')

            self._assert_dump_to_yaml(tmpdir, ystring, "test1")

        def test_all2(self, tmpdir):
            ystring = dedent(f'''\
            {FIELDS}:
              - english
              - german
            {GUID}: sdfhfghsvsdv
            {NOTE_MODEL}: LL Test
            {TAGS}:
              - marked
            ''')

            self._assert_dump_to_yaml(tmpdir, ystring, "test2")

        def test_no_note_model(self, tmpdir):
            ystring = dedent(f'''\
            {FIELDS}:
              - first
            {GUID}: '12345'
            {TAGS}:
              - noun
              - other
            ''')

            self._assert_dump_to_yaml(tmpdir, ystring, "no_note_model")

        def test_no_tags(self, tmpdir):
            for num, note in enumerate(["no_tags1", "no_tags2"]):
                ystring = dedent(f'''\
                {FIELDS}:
                  - first
                {GUID}: '12345'
                {NOTE_MODEL}: model_name
                ''')

                self._assert_dump_to_yaml(tmpdir.mkdir(str(num)), ystring, note)

    class TestNoteGrouping:
        @staticmethod
        def _assert_dump_to_yaml(tmpdir, ystring, note_grouping_name):
            file = TestDumpToYaml._make_temp_file(tmpdir)

            note = NoteGrouping.from_dict(working_note_groupings[note_grouping_name])
            note.dump_to_yaml(str(file))

            assert file.read() == ystring

        def test_nothing_grouped(self, tmpdir):
            ystring = dedent(f'''\
            {NOTES}:
              - {FIELDS}:
                  - first
                {GUID}: '12345'
                {NOTE_MODEL}: model_name
                {TAGS}:
                  - noun
                  - other
              - {FIELDS}:
                  - english
                  - german
                {GUID}: sdfhfghsvsdv
                {NOTE_MODEL}: LL Test
                {TAGS}:
                  - marked
            ''')

            self._assert_dump_to_yaml(tmpdir, ystring, "nothing_grouped")

        def test_note_model_grouped(self, tmpdir):
            ystring = dedent(f'''\
            {NOTE_MODEL}: model_name
            {NOTES}:
              - {FIELDS}:
                  - first
                {GUID}: '12345'
                {TAGS}:
                  - noun
                  - other
              - {FIELDS}:
                  - second
                {GUID}: '67890'
                {TAGS}:
                  - noun
                  - other
            ''')

            self._assert_dump_to_yaml(tmpdir, ystring, "note_model_grouped")

        def test_note_tags_grouped(self, tmpdir):
            ystring = dedent(f'''\
            {TAGS}:
              - noun
              - other
            {NOTES}:
              - {FIELDS}:
                  - first
                {GUID}: '12345'
                {NOTE_MODEL}: model_name
              - {FIELDS}:
                  - first
                {GUID}: '12345'
                {NOTE_MODEL}: model_name
            ''')

            self._assert_dump_to_yaml(tmpdir, ystring, "tags_grouped")

        def test_note_model_and_tags_grouped(self, tmpdir):
            ystring = dedent(f'''\
            {NOTE_MODEL}: model_name
            {TAGS}:
              - noun
              - other
            {NOTES}:
              - {FIELDS}:
                  - first
                {GUID}: '12345'
              - {FIELDS}:
                  - first
                {GUID}: '12345'
            ''')

            self._assert_dump_to_yaml(tmpdir, ystring, "model_and_tags_grouped")

    class TestDeckPartNotes:
        @staticmethod
        def _assert_dump_to_yaml(tmpdir, ystring, groups: list):
            file = TestDumpToYaml._make_temp_file(tmpdir)

            note = DeckPartNotes.from_dict({NOTE_GROUPINGS: [working_note_groupings[name] for name in groups]})
            note.dump_to_yaml(str(file))

            assert file.read() == ystring

        def test_two_groupings(self, tmpdir):
            ystring = dedent(f'''\
            {NOTE_GROUPINGS}:
              - {NOTE_MODEL}: model_name
                {TAGS}:
                  - noun
                  - other
                {NOTES}:
                  - {FIELDS}:
                      - first
                    {GUID}: '12345'
                  - {FIELDS}:
                      - first
                    {GUID}: '12345'
              - {NOTES}:
                  - {FIELDS}:
                      - first
                    {GUID}: '12345'
                    {NOTE_MODEL}: model_name
                    {TAGS}:
                      - noun
                      - other
                  - {FIELDS}:
                      - english
                      - german
                    {GUID}: sdfhfghsvsdv
                    {NOTE_MODEL}: LL Test
                    {TAGS}:
                      - marked
            ''')

            self._assert_dump_to_yaml(tmpdir, ystring, ["model_and_tags_grouped", "nothing_grouped"])


class TestFunctionality:
    class TestNoteGrouping:
        class TestGetAllNotes:
            def test_nothing_grouped(self):
                group = NoteGrouping.from_dict(working_note_groupings["nothing_grouped"])
                notes = group.get_all_notes()
                assert len(notes) == 2

            def test_model_grouped(self):
                group = NoteGrouping.from_dict(working_note_groupings["note_model_grouped"])
                assert group.note_model == "model_name"
                assert all([note.note_model is None for note in group.notes])

                notes = group.get_all_notes()
                assert {note.note_model for note in notes} == {"model_name"}

            def test_tags_grouped(self):
                group = NoteGrouping.from_dict(working_note_groupings["tags_grouped"])
                assert group.tags == ["noun", "other"]
                assert all([note.tags is None or note.tags == [] for note in group.notes])

                notes = group.get_all_notes()
                assert all([note.tags == ["noun", "other"] for note in notes])

            def test_tags_grouped_as_addition(self):
                group = NoteGrouping.from_dict(working_note_groupings["tags_grouped_as_addition"])
                assert group.tags == ["test", "recent"]
                assert all([note.tags is not None for note in group.notes])

                notes = group.get_all_notes()
                assert notes[0].tags == ['noun', 'other', "test", "recent"]
                assert notes[1].tags == ['marked', "test", "recent"]

            def test_no_tags(self):
                group = NoteGrouping.from_dict(working_note_groupings["tags_grouped"])
                group.tags = None
                assert all([note.tags is None or note.tags == [] for note in group.notes])

                notes = group.get_all_notes()
                assert all([note.tags == [] for note in notes])
