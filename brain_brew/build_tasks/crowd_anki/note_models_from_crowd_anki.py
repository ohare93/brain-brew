from dataclasses import dataclass, field
from typing import Optional, Union, List
import logging

from brain_brew.representation.build_config.build_task import DeckPartBuildTask
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiJsonWrapper
from brain_brew.representation.transformers.base_deck_part_from import BaseDeckPartsFrom
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.note_model_repr import NoteModel


@dataclass
class NoteModelsFromCrowdAnki(BaseDeckPartsFrom, DeckPartBuildTask):
    @classmethod
    def task_regex(cls) -> str:
        return r'note_models_from_crowd_anki'

    @classmethod
    def yamale_validator_and_deps(cls) -> (str, set):
        return f'''\
            {cls.task_regex()}:
              source: str()
              part_id: str()
              model_name: str(required=False)
              save_to_file: str(required=False)
        '''

    class Representation(BaseDeckPartsFrom.Representation):
        source: str
        model_name: Optional[str] = field(default_factory=lambda: None)
        # TODO: fields: Optional[List[str]]
        # TODO: templates: Optional[List[str]]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            ca_export=CrowdAnkiExport.create_or_get(rep.source),
            part_id=rep.part_id,
            model_name=rep.model_name or rep.part_id,
            save_to_file=rep.save_to_file
        )

    ca_export: CrowdAnkiExport
    model_name: str

    def execute(self):
        ca_wrapper: CrowdAnkiJsonWrapper = self.ca_export.json_data

        note_models_dict = {model.name: model for model in ca_wrapper.note_models}

        if self.model_name not in note_models_dict:
            raise ReferenceError(f"Missing Note Model '{self.model_name}' in CrowdAnki file")

        deck_part = NoteModel.from_crowdanki(note_models_dict[self.model_name])
        DeckPartHolder.override_or_create(self.part_id, self.save_to_file, deck_part)
