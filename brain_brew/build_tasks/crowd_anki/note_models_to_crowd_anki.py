from dataclasses import dataclass
from typing import Optional, Union, List
import logging

from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiJsonWrapper
from brain_brew.representation.transformers.base_deck_part_from import BaseDeckPartsFrom
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.note_model_repr import NoteModel


@dataclass
class NoteModelsToCrowdAnki:
    @dataclass
    class NoteModelListItem:
        @dataclass
        class Representation(RepresentationBase):
            deck_part: str
            # TODO: fields: Optional[List[str]]
            # TODO: templates: Optional[List[str]]

        @classmethod
        def from_repr(cls, data: Union[Representation, dict, str]):
            rep: cls.Representation
            if isinstance(data, cls.Representation):
                rep = data
            elif isinstance(data, dict):
                rep = cls.Representation.from_dict(data)
            else:
                rep = cls.Representation(deck_part=data)  # Support string

            return cls(
                deck_part=DeckPartHolder.from_deck_part_pool(rep.deck_part)
            )

        deck_part: NoteModel

    @dataclass
    class Representation(RepresentationBase):
        deck_parts: List[Union[dict, str]]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict, List[str]]):
        rep: cls.Representation
        if isinstance(data, cls.Representation):
            rep = data
        elif isinstance(data, dict):
            rep = cls.Representation.from_dict(data)
        else:
            rep = cls.Representation(deck_parts=data)  # Support list of Note Models

        return cls(
            note_models=list(map(cls.NoteModelListItem.from_repr, rep.deck_parts))
        )

    note_models: List[NoteModelListItem]

    def execute(self) -> List[dict]:
        return [model.deck_part.encode_as_crowdanki() for model in self.note_models]
