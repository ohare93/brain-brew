from dataclasses import dataclass, field
from typing import List, Union

from brain_brew.configuration.anki_field import AnkiField
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.interfaces.yamale_verifyable import YamlRepr

NAME = AnkiField("name")
ORDINAL = AnkiField("ord", "ordinal")
FONT = AnkiField("font", default_value="Liberation Sans")
MEDIA = AnkiField("media", default_value=[])
IS_RIGHT_TO_LEFT = AnkiField("rtl", "is_right_to_left", default_value=False)
FONT_SIZE = AnkiField("size", "font_size", default_value=20)
IS_STICKY = AnkiField("sticky", "is_sticky", default_value=False)


@dataclass
class Field(RepresentationBase, YamlRepr):
    @classmethod
    def task_name(cls) -> str:
        return r"note_model_field"

    @classmethod
    def yamale_schema(cls) -> str:
        return f"""\
            name: str()
            font: str(required=False)
            font_size: int(required=False)
            is_sticky: bool(required=False)
            is_right_to_left: bool(required=False)
        """

    @classmethod
    def from_repr(cls, data: dict):
        return cls.from_dict(data)

    @dataclass
    class CrowdAnki(RepresentationBase):
        name: str
        ord: int = field(default=None)
        font: str = field(default=FONT.default_value)
        media: List[str] = field(default_factory=lambda: MEDIA.default_value)
        rtl: bool = field(default=IS_RIGHT_TO_LEFT.default_value)
        size: int = field(default=FONT_SIZE.default_value)
        sticky: bool = field(default=IS_STICKY.default_value)

    name: str
    font: str = field(default=FONT.default_value)
    is_right_to_left: bool = field(default=IS_RIGHT_TO_LEFT.default_value)
    font_size: int = field(default=FONT_SIZE.default_value)
    is_sticky: bool = field(default=IS_STICKY.default_value)
    media: List[str] = field(default_factory=lambda: MEDIA.default_value)  # Unused in Anki

    @classmethod
    def from_crowd_anki(cls, data: Union[CrowdAnki, dict]):
        ca: cls.CrowdAnki = data if isinstance(data, cls.CrowdAnki) else cls.CrowdAnki.from_dict(data)
        return cls(
            name=ca.name, font=ca.font, media=ca.media,
            is_right_to_left=ca.rtl, font_size=ca.size, is_sticky=ca.sticky
        )

    def encode_as_crowdanki(self, ordinal: int) -> dict:
        data_dict = {
            FONT.anki_name: self.font,
            MEDIA.anki_name: self.media,
            NAME.anki_name: self.name,
            ORDINAL.anki_name: ordinal,
            IS_RIGHT_TO_LEFT.anki_name: self.is_right_to_left,
            FONT_SIZE.anki_name: self.font_size,
            IS_STICKY.anki_name: self.is_sticky
        }

        return data_dict

    def encode_as_part(self) -> dict:
        data_dict = {
            NAME.name: self.name
        }

        FONT.append_name_if_differs(data_dict, self.font)
        MEDIA.append_name_if_differs(data_dict, self.media)
        IS_RIGHT_TO_LEFT.append_name_if_differs(data_dict, self.is_right_to_left)
        FONT_SIZE.append_name_if_differs(data_dict, self.font_size)
        IS_STICKY.append_name_if_differs(data_dict, self.is_sticky)

        return data_dict
