from dataclasses import dataclass, field
from typing import Optional, Union, List

from brain_brew.commands.run_recipe.build_task import BuildPartTask
from brain_brew.configuration.part_holder import PartHolder
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.representation.generic.html_file import HTMLFile
from brain_brew.representation.yaml.note_model import NoteModel
from brain_brew.representation.yaml.note_model_field import Field
from brain_brew.representation.yaml.note_model_template import Template


@dataclass
class NoteModelFromHTMLParts(BuildPartTask):
    @classmethod
    def task_name(cls) -> str:
        return r'note_model_from_html_parts'

    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            part_id: str()
            model_id: str()
            css_file: str()
            fields: list(include('{Field.task_name()}'))
            templates: list(str())
            model_name: str(required=False)
            save_to_file: str(required=False)
        '''

    @classmethod
    def yamale_dependencies(cls) -> set:
        return {Field}

    @dataclass
    class Representation(RepresentationBase):
        part_id: str
        model_id: str
        css_file: str
        fields: List[dict]
        templates: List[dict]
        model_name: Optional[str] = field(default=None)
        save_to_file: Optional[str] = field(default=None)

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            part_id=rep.part_id,
            model_id=rep.model_id,
            css=HTMLFile.create_or_get(rep.css_file).get_data(deep_copy=True),
            fields=list(map(Field.from_dict, rep.fields)),
            templates=list(holder.part for holder in map(PartHolder.from_file_manager, rep.templates)),
            model_name=rep.model_name or rep.part_id,
            save_to_file=rep.save_to_file
        )

    part_id: str
    model_id: str
    css: str
    fields: List[Field]
    templates: List[Template]
    model_name: str
    save_to_file: Optional[str]

    def execute(self):
        part = NoteModel(
            name=self.model_name,
            id=self.model_id,
            css=self.css,
            fields=self.fields,
            templates=self.templates,
            required_fields_per_template=[]
        )

        PartHolder.override_or_create(self.part_id, self.save_to_file, part)
