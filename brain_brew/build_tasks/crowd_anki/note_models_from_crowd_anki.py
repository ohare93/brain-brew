from dataclasses import dataclass, field
from typing import Optional, Union, List
import logging

from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiJsonWrapper
from brain_brew.representation.transformers.base_deck_part_from import BaseDeckPartsFrom
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.note_model_repr import NoteModel


@dataclass
class NoteModelsFromCrowdAnki:
    @dataclass
    class NoteModelListItem(BaseDeckPartsFrom):
        @dataclass
        class Representation(BaseDeckPartsFrom.Representation):
            model_name: Optional[str] = field(default_factory=lambda: None)
            # TODO: fields: Optional[List[str]]
            # TODO: templates: Optional[List[str]]

        @classmethod
        def from_repr(cls, data: Union[Representation, dict]):
            rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
            return cls(
                name=rep.name,
                model_name=rep.model_name or rep.name,
                save_to_file=rep.save_to_file
            )

        model_name: str

    @classmethod
    def from_list(cls, note_model_items: List[dict]):
        return cls(
            note_model_items=list(map(cls.NoteModelListItem.from_repr, note_model_items))
        )

    note_model_items: List[NoteModelListItem]

    def execute(self, ca_wrapper: CrowdAnkiJsonWrapper) -> List[NoteModel]:
        note_models = {model["name"]: model for model in ca_wrapper.note_models}

        extra_models = list(note_models.keys())
        dp_note_models: List[NoteModel] = []

        for nm_item in self.note_model_items:
            if nm_item.model_name not in note_models:
                raise ReferenceError(f"Missing Note Model '{nm_item.model_name}' in CrowdAnki file")

            model = note_models[nm_item.model_name]
            extra_models.remove(nm_item.model_name)

            deck_part = NoteModel.from_crowdanki(model)
            DeckPartHolder.override_or_create(nm_item.name, nm_item.save_to_file, deck_part)

            dp_note_models.append(deck_part)

        if extra_models:
            logging.warning(f"Note Models were converted to Deck Parts, but did you miss some? Possible missing: {extra_models}")

        return dp_note_models
