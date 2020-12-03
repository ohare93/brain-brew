from dataclasses import dataclass, field
from typing import Union

from brain_brew.build_tasks.crowd_anki.headers_from_crowdanki import headers_default_values
from brain_brew.configuration.part_holder import PartHolder
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.representation.yaml.headers import Headers


@dataclass
class HeadersToCrowdAnki:
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

