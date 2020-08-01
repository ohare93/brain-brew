import pytest

from brain_brew.representation.crowdanki.crowdanki_note_model import CrowdAnkiNoteModel
from brain_brew.representation.json.json_file import JsonFile
from tests.test_files import TestFiles


class TestCrowdAnkiNoteModel:
    def test_constructor(self):
        json_data = JsonFile.read_file(TestFiles.NoteModels.LL_WORD_COMPLETE)
        model = CrowdAnkiNoteModel.from_dict(json_data)
        assert isinstance(model, CrowdAnkiNoteModel)
