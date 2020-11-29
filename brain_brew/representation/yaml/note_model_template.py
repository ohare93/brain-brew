from dataclasses import dataclass, field
from typing import Optional, Union, Set

from brain_brew.representation.configuration.anki_field import AnkiField
from brain_brew.representation.configuration.representation_base import RepresentationBase
from brain_brew.representation.yaml.yaml_object import YamlObject
from brain_brew.utils import find_media_in_field

NAME = AnkiField("name")
ORDINAL = AnkiField("ord", "ordinal")
QUESTION_FORMAT = AnkiField("qfmt", "question_format")
ANSWER_FORMAT = AnkiField("afmt", "answer_format")
BROWSER_ANSWER_FORMAT = AnkiField("bafmt", "browser_answer_format", default_value="")
BROWSER_QUESTION_FORMAT = AnkiField("bqfmt", "browser_question_format", default_value="")
DECK_OVERRIDE_ID = AnkiField("did", "deck_override_id", default_value=None)


@dataclass
class Template(RepresentationBase, YamlObject):
    @classmethod
    def from_yaml_file(cls, filename: str) -> 'Template':
        return cls.from_dict(cls.read_to_dict(filename))

    @dataclass
    class CrowdAnki(RepresentationBase):
        name: str
        qfmt: str
        afmt: str
        bqfmt: str = field(default=BROWSER_QUESTION_FORMAT.default_value)
        bafmt: str = field(default=BROWSER_ANSWER_FORMAT.default_value)
        ord: int = field(default=None)
        did: Optional[int] = field(default=None)

    name: str
    question_format: str
    answer_format: str
    question_format_in_browser: str = field(default=BROWSER_QUESTION_FORMAT.default_value)
    answer_format_in_browser: str = field(default=BROWSER_ANSWER_FORMAT.default_value)
    deck_override_id: Optional[int] = field(default=DECK_OVERRIDE_ID.default_value)

    @classmethod
    def from_crowdanki(cls, data: Union[CrowdAnki, dict]):
        ca: cls.CrowdAnki = data if isinstance(data, cls.CrowdAnki) else cls.CrowdAnki.from_dict(data)
        return cls(
            name=ca.name, question_format=ca.qfmt, answer_format=ca.afmt,
            question_format_in_browser=ca.bqfmt, answer_format_in_browser=ca.bafmt, deck_override_id=ca.did
        )

    def get_all_media_references(self) -> Set[str]:
        all_media = set()\
            .union(find_media_in_field(self.question_format))\
            .union(find_media_in_field(self.answer_format))\
            .union(find_media_in_field(self.question_format_in_browser))\
            .union(find_media_in_field(self.answer_format_in_browser))
        return all_media

    def encode_as_crowdanki(self, ordinal: int) -> dict:
        data_dict = {
            ANSWER_FORMAT.anki_name: self.answer_format,
            BROWSER_ANSWER_FORMAT.anki_name: self.answer_format_in_browser,
            BROWSER_QUESTION_FORMAT.anki_name: self.question_format_in_browser,
            DECK_OVERRIDE_ID.anki_name: self.deck_override_id,
            NAME.anki_name: self.name,
            ORDINAL.anki_name: ordinal,
            QUESTION_FORMAT.anki_name: self.question_format,
        }

        return data_dict

    def encode(self) -> dict:
        data_dict = {
            NAME.name: self.name,
            QUESTION_FORMAT.name: self.question_format,
            ANSWER_FORMAT.name: self.answer_format
        }

        BROWSER_QUESTION_FORMAT.append_name_if_differs(data_dict, self.question_format_in_browser)
        BROWSER_ANSWER_FORMAT.append_name_if_differs(data_dict, self.answer_format_in_browser)
        DECK_OVERRIDE_ID.append_name_if_differs(data_dict, self.deck_override_id)

        return data_dict
