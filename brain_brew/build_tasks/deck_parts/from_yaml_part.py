from abc import ABCMeta
from dataclasses import dataclass
from typing import Union

from brain_brew.configuration.build_config.build_task import BuildPartTask
from brain_brew.configuration.part_holder import PartHolder
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.representation.yaml.headers import Headers
from brain_brew.representation.yaml.media_group import MediaGroup
from brain_brew.representation.yaml.note_model import NoteModel
from brain_brew.representation.yaml.notes import Notes
from brain_brew.representation.yaml.yaml_object import YamlObject


@dataclass
class FromYamlPartBase(BuildPartTask, metaclass=ABCMeta):
    part_type = None

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

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)

        return cls(
            part=PartHolder.override_or_create(
                part_id=rep.part_id, save_to_file=None, part=cls.part_type.from_yaml_file(rep.file))
        )

    def execute(self):
        pass

    part: YamlObject


@dataclass
class NotesFromYamlPart(FromYamlPartBase):
    @classmethod
    def task_name(cls) -> str:
        return r'notes_from_yaml_part'

    part_type = Notes


@dataclass
class HeadersFromYamlPart(FromYamlPartBase, BuildPartTask):
    @classmethod
    def task_name(cls) -> str:
        return r'headers_from_yaml_part'

    @classmethod
    def task_regex(cls) -> str:
        return r'header[s]?_from_yaml_part'

    part_type = Headers


@dataclass
class NoteModelsFromYamlPart(FromYamlPartBase, BuildPartTask):
    @classmethod
    def task_name(cls) -> str:
        return r'note_models_from_yaml_part'

    @classmethod
    def task_regex(cls) -> str:
        return r'note_model[s]?_from_yaml_part'

    part_type = NoteModel


@dataclass
class MediaGroupFromYamlPart(FromYamlPartBase, BuildPartTask):
    @classmethod
    def task_name(cls) -> str:
        return r'media_group_from_yaml_part'

    part_type = MediaGroup
