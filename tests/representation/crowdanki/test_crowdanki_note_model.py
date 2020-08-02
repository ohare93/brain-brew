import pytest

from brain_brew.representation.crowdanki.crowdanki_note_model import CrowdAnkiNoteModel
from brain_brew.representation.json.json_file import JsonFile
from tests.test_files import TestFiles


class TestCrowdAnkiNoteModel:
    class TestConstructor:
        def test_normal(self):
            json_data = JsonFile.read_file(TestFiles.NoteModels.LL_WORD_COMPLETE)
            model = CrowdAnkiNoteModel.from_dict(json_data)
            assert isinstance(model, CrowdAnkiNoteModel)

            assert model.name == "LL Word"
            assert isinstance(model.fields, list)
            assert len(model.fields) == 7
            assert all([isinstance(field, CrowdAnkiNoteModel.Field) for field in model.fields])

            assert isinstance(model.templates, list)
            assert len(model.templates) == 7
            assert all([isinstance(template, CrowdAnkiNoteModel.Template) for template in model.templates])

        def test_only_required(self):
            json_data = JsonFile.read_file(TestFiles.NoteModels.LL_WORD_COMPLETE_ONLY_REQUIRED)
            model = CrowdAnkiNoteModel.from_dict(json_data)
            assert isinstance(model, CrowdAnkiNoteModel)
