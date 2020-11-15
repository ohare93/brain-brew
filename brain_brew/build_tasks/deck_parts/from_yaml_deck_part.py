from abc import ABCMeta
from dataclasses import dataclass
from typing import Union

from brain_brew.representation.build_config.build_task import DeckPartBuildTask
from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.headers_repr import Headers
from brain_brew.representation.yaml.media_group_repr import MediaGroup
from brain_brew.representation.yaml.note_model_repr import NoteModel
from brain_brew.representation.yaml.note_repr import Notes


@dataclass
class FromYamlDeckPartBase(DeckPartBuildTask, metaclass=ABCMeta):
    deck_part_type = None

    @classmethod
    def yamale_validator_and_deps(cls) -> (str, set):
        return f'''\
            {cls.task_regex()}:
              part_id: str()
              file: str()
            ''', None

    @dataclass
    class Representation(RepresentationBase):
        part_id: str
        file: str

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)

        return DeckPartHolder.override_or_create(
            part_id=rep.part_id, save_to_file=None, deck_part=cls.deck_part_type.from_yaml_file(rep.file))

    def execute(self):
        pass


@dataclass
class NotesFromYamlDeckPart(FromYamlDeckPartBase):
    @classmethod
    def task_regex(cls) -> str:
        return r'notes_from_yaml_deck_part'

    deck_part_type = Notes


@dataclass
class HeadersFromYamlDeckPart(FromYamlDeckPartBase):
    @classmethod
    def task_regex(cls) -> str:
        return r'headers_from_yaml_deck_part'

    deck_part_type = Headers


@dataclass
class NoteModelsFromYamlDeckPart(FromYamlDeckPartBase):
    @classmethod
    def task_regex(cls) -> str:
        return r'note_models_from_yaml_deck_part'

    deck_part_type = NoteModel


@dataclass
class MediaGroupFromYamlDeckPart(FromYamlDeckPartBase):
    @classmethod
    def task_regex(cls) -> str:
        return r'media_group_from_yaml_deck_part'

    deck_part_type = MediaGroup
