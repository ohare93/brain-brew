from dataclasses import dataclass
from typing import Optional, Union

from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiJsonWrapper
from brain_brew.representation.transformers.base_deck_part_from import BaseDeckPartsFrom
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.headers_repr import Headers
from brain_brew.transformers.transform_crowdanki import TransformCrowdAnki


@dataclass
class HeadersFromCrowdAnki(BaseDeckPartsFrom):
    @dataclass
    class Representation(BaseDeckPartsFrom.Representation):
        pass

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            name=rep.name,
            save_to_file=rep.save_to_file
        )

    def execute(self, ca_wrapper: CrowdAnkiJsonWrapper):
        headers = Headers(TransformCrowdAnki.crowd_anki_to_headers(ca_wrapper.data))

        DeckPartHolder.override_or_create(self.name, self.save_to_file, headers)

        return headers
