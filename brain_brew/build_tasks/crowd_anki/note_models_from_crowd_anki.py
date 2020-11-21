from dataclasses import dataclass, field
from typing import Optional, Union

from brain_brew.representation.build_config.build_task import BuildPartTask
from brain_brew.representation.configuration.base_parts_from import BasePartsFrom
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiJsonWrapper
from brain_brew.representation.yaml.note_model_repr import NoteModel
from brain_brew.representation.yaml.part_holder import PartHolder


@dataclass
class NoteModelsFromCrowdAnki(BasePartsFrom, BuildPartTask):
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
        ''', None

    class Representation(BasePartsFrom.Representation):
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

        part = NoteModel.from_crowdanki(note_models_dict[self.model_name])
        PartHolder.override_or_create(self.part_id, self.save_to_file, part)
