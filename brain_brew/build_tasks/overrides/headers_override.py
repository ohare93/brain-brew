from dataclasses import dataclass
from typing import Optional, Union

from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.interfaces.yamale_verifyable import YamlRepr
from brain_brew.representation.generic.html_file import HTMLFile
from brain_brew.representation.yaml.headers import Headers


@dataclass
class HeadersOverride(YamlRepr):
    @classmethod
    def task_name(cls) -> str:
        return r"headers_override"

    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            deck_description_html_file: str(required=False)
        '''

    @dataclass
    class Representation(RepresentationBase):
        deck_description_html_file: Optional[str]

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            rep=rep,
            deck_desc_html_file=HTMLFile.create_or_get(rep.deck_description_html_file)
        )

    rep: Representation
    deck_desc_html_file: Optional[HTMLFile]

    def override(self, header: Headers):
        if self.deck_desc_html_file:
            header.description = self.deck_desc_html_file.get_data(deep_copy=True)

        return header
