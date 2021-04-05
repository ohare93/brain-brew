from dataclasses import dataclass, field
from typing import Optional, Union

from brain_brew.commands.run_recipe.build_task import BuildPartTask
from brain_brew.configuration.part_holder import PartHolder
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiJsonWrapper
from brain_brew.representation.yaml.note_model import NoteModel


@dataclass
class NoteModelSingleFromCrowdAnki(BuildPartTask):
    @classmethod
    def task_name(cls) -> str:
        return r'note_model_from_crowd_anki'

    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            part_id: str()
            source: str()
            model_name: str(required=False)
            save_to_file: str(required=False)
        '''

    @dataclass
    class Representation(RepresentationBase):
        part_id: str
        source: str
        model_name: Optional[str] = field(default=None)
        save_to_file: Optional[str] = field(default=None)
        # TODO: fields: Optional[List[str]]
        # TODO: templates: Optional[List[str]]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            rep=rep,
            ca_export=CrowdAnkiExport.create_or_get(rep.source),
            part_id=rep.part_id,
            model_name=rep.model_name or rep.part_id,
            save_to_file=rep.save_to_file
        )

    rep: Representation
    part_id: str
    ca_export: CrowdAnkiExport
    model_name: str
    save_to_file: Optional[str]

    def execute(self):
        ca_wrapper: CrowdAnkiJsonWrapper = self.ca_export.json_data

        note_models_dict = {model.get('name'): model for model in ca_wrapper.note_models}

        if self.model_name not in note_models_dict:
            raise ReferenceError(f"Missing Note Model '{self.model_name}' in CrowdAnki file")

        part = NoteModel.from_crowdanki(note_models_dict[self.model_name])
        return PartHolder.override_or_create(self.part_id, self.save_to_file, part)
