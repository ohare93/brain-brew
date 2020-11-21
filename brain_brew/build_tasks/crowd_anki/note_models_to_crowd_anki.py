from dataclasses import dataclass, field
from typing import Union, List

from brain_brew.interfaces.yamale_verifyable import YamlRepr
from brain_brew.representation.configuration.representation_base import RepresentationBase
from brain_brew.representation.yaml.note_model_repr import NoteModel
from brain_brew.representation.yaml.part_holder import PartHolder


@dataclass
class NoteModelsToCrowdAnki(YamlRepr):
    @classmethod
    def task_regex(cls) -> str:
        return r'note_models_to_crowd_anki'

    @classmethod
    def yamale_validator_and_deps(cls) -> (str, set):
        return f'''\
            {cls.task_regex()}:
              parts: list(include('{cls.task_regex()}_item'))
            
            {cls.task_regex()}_item:
              part_id: str()
        ''', None

    @dataclass
    class NoteModelListItem:
        @dataclass
        class Representation(RepresentationBase):
            part_id: str
            # TODO: fields: Optional[List[str]]
            # TODO: templates: Optional[List[str]]

        @classmethod
        def from_repr(cls, data: Union[Representation, dict, str]):
            rep: cls.Representation
            if isinstance(data, cls.Representation):
                rep = data
            elif isinstance(data, dict):
                rep = cls.Representation.from_dict(data)
            else:
                rep = cls.Representation(part_id=data)  # Support string

            return cls(
                part_to_read=rep.part_id
            )

        def get_note_model(self) -> NoteModel:
            self.part = PartHolder.from_file_manager(self.part_to_read).part
            return self.part  # Todo: add filters in here

        part: NoteModel = field(init=False)
        part_to_read: str

    @dataclass
    class Representation(RepresentationBase):
        parts: List[Union[dict, str]]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict, List[str]]):
        rep: cls.Representation
        if isinstance(data, cls.Representation):
            rep = data
        elif isinstance(data, dict):
            rep = cls.Representation.from_dict(data)
        else:
            rep = cls.Representation(parts=data)  # Support list of Note Models

        note_model_items = list(map(cls.NoteModelListItem.from_repr, rep.parts))
        return cls(
            note_models=[nm.get_note_model() for nm in note_model_items]
        )

    note_models: List[NoteModel]

    def execute(self) -> List[dict]:
        return [model.encode_as_crowdanki() for model in self.note_models]
