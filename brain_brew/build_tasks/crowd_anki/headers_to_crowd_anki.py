from dataclasses import dataclass, field
from typing import Optional, Union

from brain_brew.build_tasks.crowd_anki.headers_from_crowdanki import headers_default_values
from brain_brew.interfaces.yamale_verifyable import YamlRepr
from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.representation.yaml.part_holder import PartHolder
from brain_brew.representation.yaml.headers_repr import Headers


@dataclass
class HeadersToCrowdAnki(YamlRepr):
    @classmethod
    def task_regex(cls) -> str:
        return r"headers_to_crowd_anki"

    @classmethod
    def yamale_validator_and_deps(cls) -> (str, set):
        return f'''\
            part_id: str()
        '''

    @dataclass
    class Representation(RepresentationBase):
        part_id: str

    @classmethod
    def from_repr(cls, data: Union[Representation, dict, str]):
        rep: cls.Representation
        if isinstance(data, cls.Representation):
            rep = data
        elif isinstance(data, dict):
            rep = cls.Representation.from_dict(data)
        else:
            rep = cls.Representation(part_id=data)  # Support single string being passed in

        return cls(
            headers_to_read=rep.part_id
        )

    headers_to_read: str
    headers: Headers = field(init=False)

    def execute(self) -> dict:
        self.headers = PartHolder.from_file_manager(self.headers_to_read).part

        headers = self.headers_to_crowd_anki(self.headers.data_without_name)

        return headers

    @staticmethod
    def headers_to_crowd_anki(headers_data: dict):
        return {**headers_default_values, **headers_data}

