from tempfile import NamedTemporaryFile, TemporaryDirectory

import pytest

from brain_brew.constants.deckpart_keys import DeckPartNoteKeys, NoteFlagKeys
from brain_brew.representation.json.json_file import JsonFile
from brain_brew.representation.json.deck_part_header import DeckPartHeader
from brain_brew.representation.json.deck_part_notemodel import DeckPartNoteModel, CANoteModelKeys
from brain_brew.representation.json.deck_part_notes import DeckPartNotes


def debug_write_to_target_json(data, json: JsonFile):
    json.set_data(data)
    json.write_file()


def setup_temp_file_in_folder(file_suffix):
    temp_dir = TemporaryDirectory()
    temp_file = NamedTemporaryFile(suffix=file_suffix, delete=False, dir=temp_dir.name)

    return temp_dir, temp_file


# def get_global_config(note_model_loc: str = "", group_by_note_model=True, extract_shared_tags=False) -> GlobalConfig:
#     config = GlobalConfig.get_instance(override=GlobalConfig({
#         ConfigKeys.DECK_PARTS.value: {
#             "headers": "",
#             "note_models": note_model_loc,
#             "notes": "",
#
#             ConfigKeys.DECK_PARTS_NOTES_STRUCTURE.value: {
#                 NoteFlagKeys.GROUP_BY_NOTE_MODEL.value: group_by_note_model,
#                 NoteFlagKeys.EXTRACT_SHARED_TAGS.value: extract_shared_tags
#             }
#         },
#         ConfigKeys.FLAGS.value: {
#
#         }
#     }))
#
#     return config


# @pytest.fixture()
# def global_config():
#     return GlobalConfig.get_instance(override=GlobalConfig({
#         ConfigKeys.DECK_PARTS.value: {
#             "headers": "",
#             "note_models": "",
#             "notes": "",
#
#             ConfigKeys.DECK_PARTS_NOTES_STRUCTURE.value: {
#                 NoteFlagKeys.GROUP_BY_NOTE_MODEL.value: False,
#                 NoteFlagKeys.EXTRACT_SHARED_TAGS.value: False
#             }
#         },
#         ConfigKeys.FLAGS.value: {
#
#         }
#     }))


def make_deck_part_header_mock(data):
    return DeckPartHeader(
        NamedTemporaryFile(suffix=".json", delete=False).name,
        read_now=False,
        data_override=data
    )


def make_deck_part_note_model_mock(data):
    return DeckPartNoteModel(
        NamedTemporaryFile(suffix=".json", delete=False).name,
        read_now=False,
        data_override=data
    )


def make_deck_part_notes_mock(data):
    return DeckPartNotes(
        NamedTemporaryFile(suffix=".json", delete=False).name,
        data_override=data
    )


@pytest.fixture()
def headers_mock():
    return make_deck_part_header_mock({
        "__type__": "Deck",
        "crowdanki_uuid": "72ac74b8-0077-11ea-959e-d8cb8ac9abf0",
        "deck_config_uuid": "3cc64d85-e410-11e9-960e-d8cb8ac9abf0",
        "name": "LL::1. Vocab"
    })


@pytest.fixture()
def note_models_mock():
    return [
        make_deck_part_note_model_mock({
            "__type__": "NoteModel",
            CANoteModelKeys.ID.value: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            CANoteModelKeys.NAME.value: "LL Word",
            CANoteModelKeys.FIELDS.value: [
                {"name": "Word"},
                {"name": "OtherWord"}
            ]
        }),
        make_deck_part_note_model_mock({
            "__type__": "NoteModel",
            CANoteModelKeys.ID.value: "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
            CANoteModelKeys.NAME.value: "LL Verb",
            CANoteModelKeys.FIELDS.value: [
                {"name": "Word"},
                {"name": "OtherWord"}
            ]
        }),
        make_deck_part_note_model_mock({
            "__type__": "NoteModel",
            CANoteModelKeys.ID.value: "cccccccc-cccc-cccc-cccc-cccccccccccc",
            CANoteModelKeys.NAME.value: "LL Noun",
            CANoteModelKeys.FIELDS.value: [
                {"name": "Word"},
                {"name": "OtherWord"}
            ]
        }),
        make_deck_part_note_model_mock({
            "__type__": "NoteModel",
            CANoteModelKeys.ID.value: "dddddddd-dddd-dddd-dddd-dddddddddddd",
            CANoteModelKeys.NAME.value: "LL Sentence",
            CANoteModelKeys.FIELDS.value: [
                {"name": "Word"},
                {"name": "OtherWord"}
            ]
        }),
        make_deck_part_note_model_mock({
            "__type__": "NoteModel",
            CANoteModelKeys.ID.value: "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee",
            CANoteModelKeys.NAME.value: "Cloze",
            CANoteModelKeys.FIELDS.value: [
                {"name": "Word"},
                {"name": "OtherWord"}
            ]
        })
    ]


@pytest.fixture()
def notes_mock():
    return make_deck_part_notes_mock({
        DeckPartNoteKeys.FLAGS.value: {
            NoteFlagKeys.GROUP_BY_NOTE_MODEL.value: True,
            NoteFlagKeys.EXTRACT_SHARED_TAGS.value: False
        },
        "Cloze": {
            "notes": [
                {
                    "fields": [
                        "testers",
                        ""
                    ],
                    "guid": "wpoGpMPjTD",
                    "tags": []
                }
            ]
        },
        "LL Noun": {
            "notes": [
                {
                    "fields": [
                        "banana",
                        "en banan",
                        "banano",
                        "<img src=\"All-About-Bananas-Nutrition-Facts-Health-Benefits-Recipes-and-More-RM-722x406.jpg\">",
                        "",
                        "[sound:pronunciation_da_banan.mp3]",
                        ""
                    ],
                    "guid": "sZGs^rTTEr",
                    "tags": [
                        "LL::Grammar::Noun"
                    ]
                }
            ]
        },
        "LL Sentence": {
            "notes": [
                {
                    "fields": [
                        "I don't understand",
                        "<img src=\".jpg (50)\">",
                        "y",
                        "y",
                        "jeg",
                        "forstår",
                        "ikke",
                        "",
                        "",
                        "mi ne",
                        "komprenas",
                        "",
                        "",
                        ""
                    ],
                    "guid": "x+JWF/d?][",
                    "tags": [
                        "CardType::Sentence"
                    ]
                }
            ]
        },
        "LL Verb": {
            "notes": [
                {
                    "fields": [
                        "to learn",
                        "at lære",
                        "lerni",
                        "<img src=\".jpg (51)\">",
                        "",
                        "[sound:pronunciation_da_lære.mp3]",
                        "",
                        "lærer",
                        "lærte",
                        "har lært"
                    ],
                    "guid": "ONZwlFyCX1",
                    "tags": [
                        "LL::Grammar::Verb"
                    ]
                }
            ]
        },
        "LL Word": {
            "notes": [
                {
                    "fields": [
                        "you",
                        "du",
                        "vi",
                        "<img src=\"You1.gif\">",
                        "Singular, Subjective, 2nd Person",
                        "[sound:pronunciation_da_du.mp3]",
                        ""
                    ],
                    "guid": "uZFR2ToY#3",
                    "tags": [
                        "LL::Grammar::Pronoun"
                    ]
                },
                {
                    "fields": [
                        "want",
                        "test",
                        "",
                        "",
                        "",
                        "",
                        ""
                    ],
                    "guid": "zlEirC^ya/",
                    "tags": [
                        "LL::Grammar::Noun",
                        "LL::Grammar::Verb"
                    ]
                }
            ]
        }
    })
