from dataclasses import dataclass
from typing import Optional, Union, List
import logging

from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiJsonWrapper
from brain_brew.representation.transformers.base_deck_part_from import BaseDeckPartsFrom
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.note_model_repr import NoteModel


@dataclass
class NoteModelsToCrowdAnki:
    @classmethod
    def from_list(cls, note_model_items: List[str]):
        return cls(
            note_models=list(map(DeckPartHolder.from_deck_part_pool, note_model_items))
        )

    note_models: List[NoteModel]

    def execute(self) -> List[dict]:
        return [model.encode_as_crowdanki() for model in self.note_models]
