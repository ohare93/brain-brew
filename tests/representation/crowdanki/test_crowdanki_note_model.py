import pytest

from brain_brew.representation.crowdanki.crowdanki_note_model import CrowdAnkiNoteModel, Template, Field
from brain_brew.representation.json.json_file import JsonFile
from tests.test_files import TestFiles


@pytest.fixture
def ca_nm_data_word():
    return JsonFile.read_file(TestFiles.NoteModels.LL_WORD_COMPLETE)


@pytest.fixture
def ca_nm_data_word_required_only():
    return JsonFile.read_file(TestFiles.NoteModels.LL_WORD_COMPLETE_ONLY_REQUIRED)


class TestCrowdAnkiNoteModel:
    class TestConstructor:
        def test_normal(self, ca_nm_data_word):
            model = CrowdAnkiNoteModel.from_crowdanki(ca_nm_data_word)
            assert isinstance(model, CrowdAnkiNoteModel)

            assert model.name == "LL Word"
            assert isinstance(model.fields, list)
            assert len(model.fields) == 7
            assert all([isinstance(field, Field) for field in model.fields])

            assert isinstance(model.templates, list)
            assert len(model.templates) == 7
            assert all([isinstance(template, Template) for template in model.templates])

        def test_only_required(self, ca_nm_data_word_required_only):
            model = CrowdAnkiNoteModel.from_crowdanki(ca_nm_data_word_required_only)
            assert isinstance(model, CrowdAnkiNoteModel)

        def test_manual_construction(self):
            model = CrowdAnkiNoteModel(
                "name",
                "23094149+8124+91284+12984",
                "css is garbage",
                [],
                [Field(
                    "field1",
                    0
                )],
                [Template(
                    "template1",
                    0,
                    "{{Question}}",
                    "{{Answer}}"
                )]
            )

            assert isinstance(model, CrowdAnkiNoteModel)

    class TestEncodeAsCrowdAnki:
        def test_normal(self, ca_nm_data_word):
            model = CrowdAnkiNoteModel.from_crowdanki(ca_nm_data_word)

            encoded = model.encode_as_crowdanki()

            assert encoded == ca_nm_data_word

        def test_only_required_uses_defaults(self, ca_nm_data_word, ca_nm_data_word_required_only):
            model = CrowdAnkiNoteModel.from_crowdanki(ca_nm_data_word_required_only)

            encoded = model.encode_as_crowdanki()

            assert encoded != ca_nm_data_word_required_only
            assert encoded == ca_nm_data_word
