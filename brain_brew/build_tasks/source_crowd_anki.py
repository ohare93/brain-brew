import logging
from enum import Enum

from brain_brew.build_tasks.build_task_generic import BuildTaskEnum, BuildTaskGeneric, BuildConfigKeys
from brain_brew.constants.crowdanki_keys import CAKeys
from brain_brew.constants.deckpart_keys import DeckPartNoteKeys
from brain_brew.helper.helperfunctions import blank_str_if_none
from brain_brew.representation.configuration.yaml_file import ConfigKey, YamlFile
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.representation.json.deck_part_header import DeckPartHeader
from brain_brew.representation.json.deck_part_notemodel import CANoteModelKeys, DeckPartNoteModel
from brain_brew.representation.json.deck_part_notes import CANoteKeys, DeckPartNotes


class CrowdAnkiKeys(Enum):
    FILE = "file"
    NOTE_SORT_ORDER = "note_sort_order"
    MEDIA = "media"
    USELESS_NOTE_KEYS = "useless_note_keys"


class SourceCrowdAnki(YamlFile, BuildTaskGeneric):
    @staticmethod
    def get_build_keys():
        return [
            BuildTaskEnum("deck_parts_to_crowdanki", SourceCrowdAnki, "deck_parts_to_source", "source_to_deck_parts"),
            BuildTaskEnum("crowdanki_to_deck_parts", SourceCrowdAnki, "source_to_deck_parts", "deck_parts_to_source"),
        ]

    config_entry = {}
    expected_keys = {
        BuildConfigKeys.NOTES.value: ConfigKey(True, str, None),
        BuildConfigKeys.HEADERS.value: ConfigKey(True, str, None),

        CrowdAnkiKeys.FILE.value: ConfigKey(True, str, None),
        CrowdAnkiKeys.NOTE_SORT_ORDER.value: ConfigKey(False, list, None),
        CrowdAnkiKeys.MEDIA.value: ConfigKey(True, bool, None),
        CrowdAnkiKeys.USELESS_NOTE_KEYS.value: ConfigKey(True, dict, None)
    }
    subconfig_filter = None

    headers: DeckPartHeader
    notes: DeckPartNotes

    crowd_anki_export: CrowdAnkiExport
    should_handle_media: bool
    useless_note_keys: dict

    def __init__(self, config_data: dict, read_now=True):
        self.setup_config_with_subconfig_replacement(config_data)
        self.verify_config_entry()

        self.headers = DeckPartHeader.create(self.config_entry[BuildConfigKeys.HEADERS.value], read_now=read_now)
        self.notes = DeckPartNotes.create(self.config_entry[BuildConfigKeys.NOTES.value], read_now=read_now)
        self.crowd_anki_export = CrowdAnkiExport.create(self.config_entry[CrowdAnkiKeys.FILE.value], read_now=read_now)

        self.should_handle_media = self.config_entry[CrowdAnkiKeys.MEDIA.value]
        self.useless_note_keys = self.config_entry[CrowdAnkiKeys.USELESS_NOTE_KEYS.value]

    @classmethod
    def from_yaml(cls, yaml_file_name, read_now=True):
        config_data = YamlFile.read_file(yaml_file_name)

        return SourceCrowdAnki(config_data, read_now=read_now)

    def extract_media(self):
        raise NotImplementedError

    def notes_to_deck_parts(self, notes_json, note_models_id_name_dict):
        for note in notes_json:
            for key in self.useless_note_keys:
                if key in note:
                    del note[key]
                # TODO: else raise error?

            if note[CANoteKeys.NOTE_MODEL.value] in note_models_id_name_dict:
                note[DeckPartNoteKeys.NOTE_MODEL.value] = note_models_id_name_dict[note[CANoteKeys.NOTE_MODEL.value]]
                del note[CANoteKeys.NOTE_MODEL.value]
            else:
                raise KeyError(f"Unknown NoteModel '{note[CANoteKeys.NOTE_MODEL.value]}'")

        return notes_json

    def source_to_deck_parts(self):
        logging.debug("--- Running: CrowdAnki Source to DeckParts ---")

        source_data = self.crowd_anki_export.get_data(deep_copy=True)

        # Headers
        header_keys_to_ignore = {CAKeys.NOTE_MODELS.value, CAKeys.NOTES.value, CAKeys.MEDIA_FILES.value}

        headers_data = {key: source_data[key] for key in source_data if key not in header_keys_to_ignore}
        self.headers.set_data(headers_data)

        # Note Models
        note_models = [
            DeckPartNoteModel.create(model[CANoteModelKeys.NAME.value], data_override=model)
            for model in source_data[CAKeys.NOTE_MODELS.value]
        ]

        note_models_id_name_dict = {model.id: model.name for model in note_models}

        # Notes
        notes_json = source_data[CAKeys.NOTES.value]
        notes_data = self.notes_to_deck_parts(notes_json, note_models_id_name_dict)

        self.notes.set_data(notes_data)

        logging.info(f"CrowdAnki - Source to Deck parts: \t# of Notes: {len(notes_json)}")

    def notes_to_source(self, note_models_dict_id_name):
        res_notes = self.notes.get_data(deep_copy=True)[DeckPartNoteKeys.NOTES.value]

        for note in res_notes:
            for key in self.useless_note_keys:
                note[key] = blank_str_if_none(self.useless_note_keys[key])

            note[CANoteKeys.NOTE_MODEL.value] = note_models_dict_id_name[note[DeckPartNoteKeys.NOTE_MODEL.value]]
            del note[DeckPartNoteKeys.NOTE_MODEL.value]

        return res_notes

    def deck_parts_to_source(self):
        logging.debug("--- Running: CrowdAnki DeckParts to Source ---")

        ca_json = {}

        # Headers
        ca_json.update(self.headers.get_data())

        # Note Models
        note_models = [DeckPartNoteModel.create(name) for name in self.notes.get_all_known_note_model_names()]

        ca_json.setdefault(CAKeys.NOTE_MODELS.value, [model.get_data() for model in note_models])

        note_models_dict_id_name = {model.name: model.id for model in note_models}

        # Notes
        ca_json.setdefault(CAKeys.NOTES.value, self.notes_to_source(note_models_dict_id_name))

        # Media Files
        media_files = []
        ca_json.setdefault(CAKeys.MEDIA_FILES.value, media_files)

        self.crowd_anki_export.set_data(ca_json)
