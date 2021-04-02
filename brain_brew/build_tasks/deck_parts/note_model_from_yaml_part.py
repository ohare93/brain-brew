from dataclasses import dataclass
from typing import Union

from brain_brew.build_tasks.deck_parts.from_yaml_part import FromYamlPartBase
from brain_brew.commands.run_recipe.build_task import BuildPartTask
from brain_brew.configuration.part_holder import PartHolder
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.representation.yaml.note_model import NoteModel


@dataclass
class NoteModelsFromYamlPart(FromYamlPartBase, BuildPartTask):
    @classmethod
    def task_name(cls) -> str:
        return r'note_models_from_yaml_part'

    @classmethod
    def task_regex(cls) -> str:
        return r'note_model[s]?_from_yaml_part'

    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            part_id: str()
            file: str()
        '''

    @dataclass
    class Representation(RepresentationBase):
        part_id: str
        file: str
        # TODO: Overrides

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)

        return cls(
            rep=rep,
            part=PartHolder.override_or_create(
                part_id=rep.part_id, save_to_file=None, part=NoteModel.from_yaml_file(rep.file))
        )

    def execute(self):
        pass
