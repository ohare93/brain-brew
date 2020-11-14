import json
import sys
from textwrap import dedent
from typing import List, Set
from ruamel.yaml import round_trip_dump
from brain_brew.representation.yaml.yaml_object import yaml_dump, yaml_load

import pytest

from brain_brew.representation.yaml.note_repr import Note, NoteGrouping, Notes, \
    NOTES, NOTE_GROUPINGS, FIELDS, GUID, NOTE_MODEL, TAGS, FLAGS

working_notes = {
    "test1": {FIELDS: ['first'], GUID: "12345", NOTE_MODEL: "model_name", TAGS: ['noun', 'other']},
    "test2": {FIELDS: ['english', 'german'], GUID: "sdfhfghsvsdv", NOTE_MODEL: "LL Test", TAGS: ['marked']},
    "no_note_model": {FIELDS: ['first'], GUID: "12345", TAGS: ['noun', 'other']},
    "no_note_model2": {FIELDS: ['second'], GUID: "67890", TAGS: ['noun', 'other']},
    "no_tags1": {FIELDS: ['first'], GUID: "12345", NOTE_MODEL: "model_name"},
    "no_tags2": {FIELDS: ['first'], GUID: "12345", NOTE_MODEL: "model_name", TAGS: []},
    "no_model_or_tags": {FIELDS: ['first'], GUID: "12345"},
    "test1_with_default_flags": {FIELDS: ['first'], GUID: "12345", NOTE_MODEL: "model_name", TAGS: ['noun', 'other'], FLAGS: 0},
    "test1_with_flags": {FIELDS: ['first'], GUID: "12345", NOTE_MODEL: "model_name", TAGS: ['noun', 'other'], FLAGS: 1},
}

working_note_groupings = {
    "nothing_grouped": {NOTES: [working_notes["test1"], working_notes["test2"]]},
    "note_model_grouped": {NOTES: [working_notes["no_note_model"], working_notes["no_note_model2"]], NOTE_MODEL: "model_name"},
    "note_model_grouped2": {NOTES: [working_notes["no_note_model"], working_notes["no_note_model2"]], NOTE_MODEL: "different_model"},
    "tags_grouped": {NOTES: [working_notes["no_tags1"], working_notes["no_tags2"]], TAGS: ["noun", "other"]},
    "tags_grouped_as_addition": {NOTES: [working_notes["test1"], working_notes["test2"]], TAGS: ["test", "recent"]},
    "model_and_tags_grouped": {NOTES: [working_notes["no_model_or_tags"], working_notes["no_model_or_tags"]], NOTE_MODEL: "model_name", TAGS: ["noun", "other"]}
}

working_dpns = {
    "one_group": {NOTE_GROUPINGS: [working_note_groupings["nothing_grouped"]]},
    "two_groups_two_models": {NOTE_GROUPINGS: [working_note_groupings["nothing_grouped"], working_note_groupings["note_model_grouped"]]},
    "two_groups_three_models": {NOTE_GROUPINGS: [working_note_groupings["nothing_grouped"], working_note_groupings["note_model_grouped2"]]},
}


@pytest.fixture(params=working_notes.values())
def note_fixtures(request):
    return Note.from_dict(request.param)


@pytest.fixture(params=working_note_groupings.values())
def note_grouping_fixtures(request):
    return NoteGrouping.from_dict(request.param)


class TestConstructor:
    class TestNote:
        @pytest.mark.parametrize("fields, guid, note_model, tags, flags, media", [
            ([], "", "", [], 0, {}),
            (None, None, None, None, None, None),
            (["test", "blah", "whatever"], "1234567890x", "model_name", ["noun"], 1, {}),
            (["test", "blah", "<img src=\"animal.jpg\">"], "1234567890x", "model_name", ["noun"], 2, {"animal.jpg"}),
        ])
        def test_constructor(self, fields: List[str], guid: str, note_model: str, tags: List[str], flags: int, media: Set[str]):
            note = Note(fields=fields, guid=guid, note_model=note_model, tags=tags, flags=flags)

            assert isinstance(note, Note)
            assert note.fields == fields
            assert note.guid == guid
            assert note.note_model == note_model
            assert note.tags == tags
            assert note.flags == flags
            # assert note.media_references == media

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
            dpn = Notes(note_groupings=[NoteGrouping.from_dict(working_note_groupings["nothing_grouped"])])
            assert isinstance(dpn, Notes)

        def test_from_dict(self):
            dpn = Notes.from_dict({NOTE_GROUPINGS: [working_note_groupings["nothing_grouped"]]})
            assert isinstance(dpn, Notes)


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

        def test_with_flags(self, tmpdir):
            ystring = dedent(f'''\
            {FIELDS}:
            - first
            {GUID}: '12345'
            {FLAGS}: 1
            {NOTE_MODEL}: model_name
            {TAGS}:
            - noun
            - other
            ''')

            self._assert_dump_to_yaml(tmpdir, ystring, "test1_with_flags")

        def test_with_default_flags(self, tmpdir):
            ystring = dedent(f'''\
            {FIELDS}:
            - first
            {GUID}: '12345'
            {NOTE_MODEL}: model_name
            {TAGS}:
            - noun
            - other
            ''')

            self._assert_dump_to_yaml(tmpdir, ystring, "test1_with_default_flags")

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

            note = Notes.from_dict({NOTE_GROUPINGS: [working_note_groupings[name] for name in groups]})
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
    class TestGetMediaReferences:
        class TestNote:
            @pytest.mark.parametrize("fields, expected_count", [
                ([], 0),
                (["nothing", "empty", "can't find nothing here"], 0),
                (["<img src=\"animal.jpg\">", "empty", "can't find nothing here"], 1),
                (["<img src=\"animal.jpg\">", "<img src=\"animal.jpg\">", "<img src=\"animal.jpg\">"], 1),
                (["<img src=\"animal.jpg\">", "<img src=\"food.jpg\">", "<img src=\"object.jpg\">"], 3),
                (["<img src=\"animal.jpg\">", "[sound:test.mp3]", "[sound:test.mp3]"], 2),
            ])
            def test_all(self, fields, expected_count):
                note = Note(fields=fields, note_model=None, guid="", tags=None, flags=0)
                media_found = note.get_all_media_references()
                assert isinstance(media_found, Set)
                assert len(media_found) == expected_count

    class TestGetAllNoteModels:
        class TestNoteGrouping:
            def test_nothing_grouped(self):
                group = NoteGrouping.from_dict(working_note_groupings["nothing_grouped"])
                models = group.get_all_known_note_model_names()
                assert models == {'LL Test', 'model_name'}

            def test_grouped(self):
                group = NoteGrouping.from_dict(working_note_groupings["note_model_grouped"])
                models = group.get_all_known_note_model_names()
                assert models == {'model_name'}

        class TestDeckPartNotes:
            def test_two_groups_two_models(self):
                dpn = Notes.from_dict(working_dpns["two_groups_two_models"])
                models = dpn.get_all_known_note_model_names()
                assert models == {'LL Test', 'model_name'}

            def test_two_groups_three_models(self):
                dpn = Notes.from_dict(working_dpns["two_groups_three_models"])
                models = dpn.get_all_known_note_model_names()
                assert models == {'LL Test', 'model_name', 'different_model'}

    # class TestGetAllNotes:
    #     class TestNoteGrouping:
    #         def test_nothing_grouped(self):
    #             group = NoteGrouping.from_dict(working_note_groupings["nothing_grouped"])
    #             notes = group.get_all_notes_copy([], False)
    #             assert len(notes) == 2
    #
    #         def test_model_grouped(self):
    #             group = NoteGrouping.from_dict(working_note_groupings["note_model_grouped"])
    #             assert group.note_model == "model_name"
    #             assert all([note.note_model is None for note in group.notes])
    #
    #             notes = group.get_all_notes_copy()
    #             assert {note.note_model for note in notes} == {"model_name"}
    #
    #         def test_tags_grouped(self):
    #             group = NoteGrouping.from_dict(working_note_groupings["tags_grouped"])
    #             assert group.tags == ["noun", "other"]
    #             assert all([note.tags is None or note.tags == [] for note in group.notes])
    #
    #             notes = group.get_all_notes_copy()
    #             assert all([note.tags == ["noun", "other"] for note in notes])
    #
    #         def test_tags_grouped_as_addition(self):
    #             group = NoteGrouping.from_dict(working_note_groupings["tags_grouped_as_addition"])
    #             assert group.tags == ["test", "recent"]
    #             assert all([note.tags is not None for note in group.notes])
    #
    #             notes = group.get_all_notes_copy()
    #             assert notes[0].tags == ['noun', 'other', "test", "recent"]
    #             assert notes[1].tags == ['marked', "test", "recent"]
    #
    #         def test_no_tags(self):
    #             group = NoteGrouping.from_dict(working_note_groupings["tags_grouped"])
    #             group.tags = None
    #             assert all([note.tags is None or note.tags == [] for note in group.notes])
    #
    #             notes = group.get_all_notes_copy()
    #             assert all([note.tags == [] for note in notes])
    #
