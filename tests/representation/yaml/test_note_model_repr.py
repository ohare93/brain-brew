import pytest

from brain_brew.representation.yaml.note_model_repr import NoteModel
from brain_brew.representation.yaml.note_model_field import Field
from brain_brew.representation.yaml.note_model_template import Template
from brain_brew.representation.json.json_file import JsonFile
from brain_brew.representation.yaml.yaml_object import YamlObject
from tests.test_files import TestFiles


# CrowdAnki Files --------------------------------------------------------------------------
from tests.test_helpers import debug_write_part_to_file


@pytest.fixture
def ca_nm_data_word():
    return JsonFile.read_file(TestFiles.CrowdAnkiNoteModels.LL_WORD_COMPLETE)


@pytest.fixture
def ca_nm_word(ca_nm_data_word) -> NoteModel:
    return NoteModel.from_crowdanki(ca_nm_data_word)


@pytest.fixture
def ca_nm_data_word_required_only():
    return JsonFile.read_file(TestFiles.CrowdAnkiNoteModels.LL_WORD_COMPLETE_ONLY_REQUIRED)


@pytest.fixture
def ca_nm_word_required_only(ca_nm_data_word_required_only) -> NoteModel:
    return NoteModel.from_crowdanki(ca_nm_data_word_required_only)


@pytest.fixture
def ca_nm_data_word_no_defaults():
    return JsonFile.read_file(TestFiles.CrowdAnkiNoteModels.LL_WORD_COMPLETE_NO_DEFAULTS)


@pytest.fixture
def ca_nm_word_no_defaults(ca_nm_data_word_no_defaults) -> NoteModel:
    return NoteModel.from_crowdanki(ca_nm_data_word_no_defaults)


# Yaml Files --------------------------------------------------------------------------
@pytest.fixture
def nm_data_word_required_only():
    return YamlObject.read_to_dict(TestFiles.NoteModels.LL_WORD_ONLY_REQUIRED)


@pytest.fixture
def nm_data_word_no_defaults():
    return YamlObject.read_to_dict(TestFiles.NoteModels.LL_WORD_NO_DEFAULTS)


class TestCrowdAnkiNoteModel:
    class TestConstructor:
        def test_normal(self, ca_nm_word):
            model = ca_nm_word
            assert isinstance(model, NoteModel)

            assert model.name == "LL Word"
            assert isinstance(model.fields, list)
            assert len(model.fields) == 7
            assert all([isinstance(field, Field) for field in model.fields])

            assert isinstance(model.templates, list)
            assert len(model.templates) == 7
            assert all([isinstance(template, Template) for template in model.templates])

        def test_only_required(self, ca_nm_word_required_only):
            model = ca_nm_word_required_only
            assert isinstance(model, NoteModel)

        def test_manual_construction(self):
            model = NoteModel(
                "name",
                "23094149+8124+91284+12984",
                "css is garbage",
                [],
                [Field(
                    "field1"
                )],
                [Template(
                    "template1",
                    "{{Question}}",
                    "{{Answer}}"
                )]
            )

            assert isinstance(model, NoteModel)

    class TestEncodeAsCrowdAnki:
        def test_normal(self, ca_nm_word, ca_nm_data_word):
            model = ca_nm_word

            encoded = model.encode_as_crowdanki()

            assert encoded == ca_nm_data_word

        def test_only_required_uses_defaults(self, ca_nm_word_required_only,
                                             ca_nm_data_word, ca_nm_data_word_required_only):
            model = ca_nm_word_required_only

            encoded = model.encode_as_crowdanki()

            assert encoded != ca_nm_data_word_required_only
            assert encoded == ca_nm_data_word

    class TestEncodeAsDeckPart:
        def test_normal(self, ca_nm_word, ca_nm_data_word, ca_nm_data_word_required_only, nm_data_word_required_only):
            model = ca_nm_word

            encoded = model.encode()

            assert encoded != ca_nm_data_word
            assert encoded != ca_nm_data_word_required_only
            assert encoded == nm_data_word_required_only

        def test_only_required_uses_defaults(self, ca_nm_word_no_defaults, ca_nm_data_word_no_defaults, nm_data_word_no_defaults):
            model = ca_nm_word_no_defaults

            encoded = model.encode()

            # debug_write_part_to_file(model, TestFiles.NoteModels.LL_WORD_NO_DEFAULTS)

            assert encoded != ca_nm_data_word_no_defaults
            assert encoded == nm_data_word_no_defaults
