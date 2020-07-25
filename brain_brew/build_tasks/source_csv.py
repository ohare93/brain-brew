from enum import Enum
from typing import List, Dict

from brain_brew.build_tasks.build_task_generic import BuildTaskGeneric
from brain_brew.constants.build_config_keys import BuildTaskEnum, BuildConfigKeys
from brain_brew.constants.deckpart_keys import DeckPartNoteKeys
from brain_brew.interfaces.verifiable import Verifiable
from brain_brew.representation.configuration.csv_file_mapping import CsvFileMapping
from brain_brew.representation.configuration.note_model_mapping import NoteModelMapping
from brain_brew.utils import single_item_to_list
from brain_brew.representation.generic.yaml_file import ConfigKey, YamlFile
from brain_brew.representation.generic.csv_file import CsvFile, CsvKeys
from brain_brew.representation.json.deck_part_notemodel import DeckPartNoteModel
from brain_brew.representation.json.deck_part_notes import DeckPartNotes


class SourceCsvKeys(Enum):
    NOTES = "notes"
    NOTE_MODEL_MAPPINGS = "note_model_mappings"
    CSV_MAPPINGS = "csv_file_mappings"


class SourceCsv(YamlFile, BuildTaskGeneric, Verifiable):
    @staticmethod
    def get_build_keys():
        return [
            BuildTaskEnum("deck_parts_to_csv_collection", SourceCsv, "deck_parts_to_source", "source_to_deck_parts"),
            BuildTaskEnum("csv_collection_to_deck_parts", SourceCsv, "source_to_deck_parts", "deck_parts_to_source"),
        ]

