import os
from dataclasses import dataclass, field
from typing import Optional, Union, Set

from brain_brew.configuration.anki_field import AnkiField
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.interfaces.yamale_verifyable import YamlRepr
from brain_brew.representation.generic.html_file import HTMLFile
from brain_brew.representation.yaml.yaml_object import YamlObject
from brain_brew.utils import find_media_in_field, split_by_regex

NAME = AnkiField("name")
ORDINAL = AnkiField("ord", "ordinal")
QUESTION_FORMAT = AnkiField("qfmt", "question_format")
ANSWER_FORMAT = AnkiField("afmt", "answer_format")
BROWSER_ANSWER_FORMAT = AnkiField("bafmt", "browser_answer_format", default_value="")
BROWSER_QUESTION_FORMAT = AnkiField("bqfmt", "browser_question_format", default_value="")
DECK_OVERRIDE_ID = AnkiField("did", "deck_override_id", default_value=None)

HTML_FILE = AnkiField("html_file")
BROWSER_HTML_FILE = AnkiField("browser_html_file", default_value=None)

html_separator_regex = r'[\n]{1,}[-]{1,}[\n]{1,}'


@dataclass
class Template(RepresentationBase, YamlObject, YamlRepr):
    @classmethod
    def task_name(cls) -> str:
        return r'note_model_template_from_html'

    @classmethod
    def yamale_schema(cls) -> str:
        return f"""\
            name: str()
            html_file: str()
            browser_html_file: str(required=False)
            deck_override_id: int(required=False)
        """

    @dataclass
    class HTML(RepresentationBase):
        name: str
        html_file: str
        browser_html_file: Optional[str] = field(default=None)
        deck_override_id: Optional[int] = field(default=DECK_OVERRIDE_ID.default_value)

    @classmethod
    def from_repr(cls, data: Union[HTML, dict]):
        rep: cls.HTML = data if isinstance(data, cls.HTML) else cls.HTML.from_dict(data)
        return cls.from_html_files(rep)

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

    html_file: Optional[str] = field(default="")
    browser_html_file: Optional[str] = field(default="")

    @classmethod
    def from_html_files(cls, data: Union[HTML, dict]):
        html_rep: cls.HTML = data if isinstance(data, cls.HTML) else cls.HTML.from_dict(data)

        html_file = HTMLFile.create_or_get(html_rep.html_file)
        browser_html_file = HTMLFile.create_or_get(html_rep.browser_html_file) if html_rep.browser_html_file else None

        main_data = html_file.get_data(deep_copy=True)
        browser_data = browser_html_file.get_data(deep_copy=True) if browser_html_file else None

        def split_template(the_data, file):
            split = split_by_regex(the_data, html_separator_regex)
            if len(split) != 2:
                raise ValueError(f"Cannot find" if len(split) < 2 else "More than one"
                                 f" separator '---' in html file '{file.file_location}'")
            return split[0], split[1]

        front, back = split_template(main_data, html_file)
        browser_front, browser_back = split_template(browser_data, browser_html_file) if browser_data else ("", "")

        return cls(
            name=html_rep.name,
            question_format=front,
            answer_format=back,
            question_format_in_browser=browser_front,
            answer_format_in_browser=browser_back,
            deck_override_id=html_rep.deck_override_id,
            html_file=html_rep.html_file,
            browser_html_file=html_rep.browser_html_file
        )

    @classmethod
    def from_crowdanki(cls, data: Union[CrowdAnki, dict]):
        ca: cls.CrowdAnki = data if isinstance(data, cls.CrowdAnki) else cls.CrowdAnki.from_dict(data)
        return cls(
            name=ca.name, question_format=ca.qfmt, answer_format=ca.afmt,
            question_format_in_browser=ca.bqfmt, answer_format_in_browser=ca.bafmt, deck_override_id=ca.did
        )

    def encode_as_part(self, folder: str):
        file = os.path.join(folder, f"{self.name}.html")
        b_file = os.path.join(folder, f"{self.name}_browser.html")

        data_dict = {
            NAME.name: self.name,
            HTML_FILE.name: file
        }

        BROWSER_HTML_FILE.append_name_if_differs(data_dict, b_file)
        DECK_OVERRIDE_ID.append_name_if_differs(data_dict, self.deck_override_id)

        return data_dict

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

    def get_all_media_references(self) -> Set[str]:
        all_media = set()\
            .union(find_media_in_field(self.question_format))\
            .union(find_media_in_field(self.answer_format))\
            .union(find_media_in_field(self.question_format_in_browser))\
            .union(find_media_in_field(self.answer_format_in_browser))
        return all_media
