from dataclasses import dataclass
from typing import Optional, Union

from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.headers_repr import Headers
from brain_brew.transformers.transform_crowdanki import TransformCrowdAnki


@dataclass
class HeadersToCrowdAnki:
    @dataclass
    class Representation(RepresentationBase):
        name: str

    @classmethod
    def from_repr(cls, data: Union[Representation, dict, str]):
        rep: cls.Representation
        if isinstance(data, cls.Representation):
            rep = data
        elif isinstance(data, dict):
            rep = cls.Representation.from_dict(data)
        else:
            rep = cls.Representation(name=data)  # Support single string being passed in

        return cls(
            headers=DeckPartHolder.from_deck_part_pool(rep.name),
        )

    headers: Headers

    def execute(self):
        headers = Headers(TransformCrowdAnki.headers_to_crowd_anki(self.headers.data))

        return headers
