import logging
from dataclasses import dataclass, field
from typing import Optional, Union

from brain_brew.commands.run_recipe.build_task import BuildPartTask
from brain_brew.configuration.part_holder import PartHolder
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiJsonWrapper
from brain_brew.representation.yaml.note_model import NoteModel


@dataclass
class NoteModelsAllFromCrowdAnki(BuildPartTask):
    @classmethod
    def task_name(cls) -> str:
        return r'note_models_all_from_crowd_anki'

    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            source: str()
        '''

    @dataclass
    class Representation(RepresentationBase):
        source: str

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            ca_export=CrowdAnkiExport.create_or_get(rep.source)
        )

    ca_export: CrowdAnkiExport

    def execute(self):
        ca_wrapper: CrowdAnkiJsonWrapper = self.ca_export.json_data

        note_models_dict = {model.get('name'): model for model in ca_wrapper.note_models}

        parts = []
        for name, model in note_models_dict.items():
            parts.append(PartHolder.override_or_create(name, None, NoteModel.from_crowdanki(model)))

        logging.info(f"Found {len(parts)} note model{'s' if len(parts) > 1 else ''} in CrowdAnki Export: '"
                     + "', '".join(note_models_dict.keys()) + "'")

        return parts
