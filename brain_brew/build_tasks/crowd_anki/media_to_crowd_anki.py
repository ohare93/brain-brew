import logging
from dataclasses import dataclass
from typing import Union, List, Set

from brain_brew.file_manager import FileManager
from brain_brew.interfaces.media_container import MediaContainer
from brain_brew.interfaces.yamale_verifyable import YamlRepr
from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.representation.generic.media_file import MediaFile
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.representation.yaml.note_model_repr import NoteModel
from brain_brew.representation.yaml.note_repr import Notes


@dataclass
class MediaToCrowdAnki(YamlRepr):
    @classmethod
    def task_regex(cls) -> str:
        return r'media_to_crowd_anki'

    @classmethod
    def yamale_validator_and_deps(cls) -> (str, set):
        return f'''\
            {cls.task_regex()}
              from_notes: bool()
              from_note_models: bool()
        '''

    @dataclass
    class Representation(RepresentationBase):

    @classmethod
    def from_repr(cls, data: Union[Representation, dict, bool]):



    # TODO: Do the actual file copying to target
    # TODO: Return set of media files
