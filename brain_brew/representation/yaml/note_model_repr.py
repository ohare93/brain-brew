from collections import OrderedDict
from dataclasses import dataclass, field
from typing import List, Optional, Union, Dict

from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.representation.yaml.my_yaml import YamlRepr
from brain_brew.utils import list_of_str_to_lowercase, find_media_in_field


class AnkiField:
    name: str
    anki_name: str
    default_value: any

    def __init__(self, anki_name, name=None, default_value=None):
        self.anki_name = anki_name
        self.name = name if name is not None else anki_name
        self.default_value = default_value

    def append_name_if_differs(self, dict_to_add_to: dict, value):
        if value != self.default_value:
            dict_to_add_to.setdefault(self.name, value)


# CrowdAnki
CROWDANKI_ID = AnkiField("crowdanki_uuid", "id")
CROWDANKI_TYPE = AnkiField("__type__", default_value="NoteModel")

# Shared
NAME = AnkiField("name")
ORDINAL = AnkiField("ord", "ordinal")

# Note Model
CSS = AnkiField("css")
LATEX_PRE = AnkiField("latexPre", "latex_pre", default_value="\\documentclass[12pt]{article}\n\\special{papersize=3in,5in}\n\\usepackage{amssymb,amsmath}\n\\pagestyle{empty}\n\\setlength{\\parindent}{0in}\n\\begin{document}\n")
LATEX_POST = AnkiField("latexPost", "latex_post", default_value="\\end{document}")
REQUIRED_FIELDS_PER_TEMPLATE = AnkiField("req", "required_fields_per_template")
FIELDS = AnkiField("flds", "fields")
TEMPLATES = AnkiField("tmpls", "templates")
TAGS = AnkiField("tags", default_value=[])
SORT_FIELD_NUM = AnkiField("sortf", "sort_field_num", default_value=0)
IS_CLOZE = AnkiField("type", "is_cloze", default_value=False)
VERSION = AnkiField("vers", "version", default_value=[])

# Field
FONT = AnkiField("font", default_value="Liberation Sans")
MEDIA = AnkiField("media", default_value=[])
IS_RIGHT_TO_LEFT = AnkiField("rtl", "is_right_to_left", default_value=False)
FONT_SIZE = AnkiField("size", default_value=20)
IS_STICKY = AnkiField("sticky", "is_sticky", default_value=False)

# Template
QUESTION_FORMAT = AnkiField("qfmt", "question_format")
ANSWER_FORMAT = AnkiField("afmt", "answer_format")
BROWSER_ANSWER_FORMAT = AnkiField("bafmt", "browser_answer_format", default_value="")
BROWSER_QUESTION_FORMAT = AnkiField("bqfmt", "browser_question_format", default_value="")
DECK_OVERRIDE_ID = AnkiField("did", "deck_override_id", default_value=None)


@dataclass
class Template(RepresentationBase):
    @dataclass
    class CrowdAnki(RepresentationBase):
        name: str
        ord: int
        qfmt: str
        afmt: str
        bqfmt: str = field(default=BROWSER_QUESTION_FORMAT.default_value)
        bafmt: str = field(default=BROWSER_ANSWER_FORMAT.default_value)
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

    def get_all_media_references(self) -> set:
        all_media = set()\
            .union(find_media_in_field(self.question_format))\
            .union(find_media_in_field(self.answer_format))\
            .union(find_media_in_field(self.question_format_in_browser))\
            .union(find_media_in_field(self.answer_format_in_browser))
        return all_media

    def encode_as_crowdanki(self, ordinal: int) -> dict:
        data_dict = {
            NAME.anki_name: self.name,
            ORDINAL.anki_name: ordinal,
            QUESTION_FORMAT.anki_name: self.question_format,
            ANSWER_FORMAT.anki_name: self.answer_format,
            BROWSER_QUESTION_FORMAT.anki_name: self.question_format_in_browser,
            BROWSER_ANSWER_FORMAT.anki_name: self.answer_format_in_browser,
            DECK_OVERRIDE_ID.anki_name: self.deck_override_id
        }

        return data_dict

    def encode_as_deck_part(self) -> dict:
        data_dict = {
            NAME.name: self.name,
            QUESTION_FORMAT.name: self.question_format,
            ANSWER_FORMAT.name: self.answer_format
        }

        BROWSER_QUESTION_FORMAT.append_name_if_differs(data_dict, self.question_format_in_browser)
        BROWSER_ANSWER_FORMAT.append_name_if_differs(data_dict, self.answer_format_in_browser)
        DECK_OVERRIDE_ID.append_name_if_differs(data_dict, self.deck_override_id)

        return data_dict


@dataclass
class Field(RepresentationBase):
    @dataclass
    class CrowdAnki(RepresentationBase):
        name: str
        ord: int
        font: str = field(default=FONT.default_value)
        media: List[str] = field(default_factory=lambda: MEDIA.default_value)
        rtl: bool = field(default=IS_RIGHT_TO_LEFT.default_value)
        size: int = field(default=FONT_SIZE.default_value)
        sticky: bool = field(default=IS_STICKY.default_value)

    name: str
    font: str = field(default=FONT.default_value)
    media: List[str] = field(default_factory=lambda: MEDIA.default_value)  # Unused in Anki
    is_right_to_left: bool = field(default=IS_RIGHT_TO_LEFT.default_value)
    font_size: int = field(default=FONT_SIZE.default_value)
    is_sticky: bool = field(default=IS_STICKY.default_value)

    @classmethod
    def from_crowdanki(cls, data: Union[CrowdAnki, dict]):
        ca: cls.CrowdAnki = data if isinstance(data, cls.CrowdAnki) else cls.CrowdAnki.from_dict(data)
        return cls(
            name=ca.name, font=ca.font, media=ca.media,
            is_right_to_left=ca.rtl, font_size=ca.size, is_sticky=ca.sticky
        )

    def encode_as_crowdanki(self, ordinal: int) -> dict:
        data_dict = {
            NAME.anki_name: self.name,
            ORDINAL.anki_name: ordinal,
            FONT.anki_name: self.font,
            MEDIA.anki_name: self.media,
            IS_RIGHT_TO_LEFT.anki_name: self.is_right_to_left,
            FONT_SIZE.anki_name: self.font_size,
            IS_STICKY.anki_name: self.is_sticky
        }

        return data_dict

    def encode_as_deck_part(self) -> dict:
        data_dict = {
            NAME.name: self.name
        }

        FONT.append_name_if_differs(data_dict, self.font)
        MEDIA.append_name_if_differs(data_dict, self.media)
        IS_RIGHT_TO_LEFT.append_name_if_differs(data_dict, self.is_right_to_left)
        FONT_SIZE.append_name_if_differs(data_dict, self.font_size)
        IS_STICKY.append_name_if_differs(data_dict, self.is_sticky)

        return data_dict


@dataclass
class NoteModel(YamlRepr, RepresentationBase):
    @dataclass
    class CrowdAnki(RepresentationBase):
        name: str
        crowdanki_uuid: str
        css: str
        req: List[list]
        flds: List[dict]
        tmpls: List[dict]
        latexPre: str = field(default=LATEX_PRE.default_value)
        latexPost: str = field(default=LATEX_POST.default_value)
        __type__: str = field(default=CROWDANKI_TYPE.default_value)
        tags: List[str] = field(default_factory=lambda: TAGS.default_value)
        sortf: int = field(default=SORT_FIELD_NUM.default_value)
        type: int = field(default=0)  # Is_Cloze Manually set to 0
        vers: list = field(default_factory=lambda: VERSION.default_value)

    name: str
    crowdanki_id: str
    css: str
    required_fields_per_template: List[list]  # TODO: Get rid of this as requirement
    fields: List[Field]
    templates: List[Template]

    latex_post: str = field(default=LATEX_PRE.default_value)
    latex_pre: str = field(default=LATEX_POST.default_value)
    sort_field_num: int = field(default=SORT_FIELD_NUM.default_value)
    is_cloze: bool = field(default=IS_CLOZE.default_value)
    crowdanki_type: str = field(default=CROWDANKI_TYPE.default_value)  # Should always be "NoteModel"
    tags: List[str] = field(default_factory=lambda: TAGS.default_value)  # Tags of the last added note
    version: list = field(default_factory=lambda: VERSION.default_value)  # Legacy version number. Deprecated in Anki

    @classmethod
    def from_crowdanki(cls, data: Union[CrowdAnki, dict]):  # TODO: field_whitelist: List[str] = None, note_model_whitelist: List[str] = None):
        ca: cls.CrowdAnki = data if isinstance(data, cls.CrowdAnki) else cls.CrowdAnki.from_dict(data)
        return cls(
            fields=[Field.from_crowdanki(f) for f in ca.flds],
            templates=[Template.from_crowdanki(t) for t in ca.tmpls],
            is_cloze=bool(ca.type),
            name=ca.name, css=ca.css, latex_pre=ca.latexPre, latex_post=ca.latexPost,
            required_fields_per_template=ca.req, tags=ca.tags, sort_field_num=ca.sortf, version=ca.vers,
            crowdanki_id=ca.crowdanki_uuid, crowdanki_type=ca.__type__
        )

    def encode_as_crowdanki(self) -> dict:
        data_dict = {
            NAME.anki_name: self.name,
            CROWDANKI_ID.anki_name: self.crowdanki_id,
            CSS.anki_name: self.css,
            REQUIRED_FIELDS_PER_TEMPLATE.anki_name: self.required_fields_per_template,
            LATEX_PRE.anki_name: self.latex_pre,
            LATEX_POST.anki_name: self.latex_post,
            SORT_FIELD_NUM.anki_name: self.sort_field_num,
            CROWDANKI_TYPE.anki_name: self.crowdanki_type,
            TAGS.anki_name: self.tags,
            VERSION.anki_name: self.version,
            IS_CLOZE.anki_name: 1 if self.is_cloze else 0
        }

        data_dict.setdefault(FIELDS.anki_name, [f.encode_as_crowdanki(num) for num, f in enumerate(self.fields)])
        data_dict.setdefault(TEMPLATES.anki_name, [t.encode_as_crowdanki(num) for num, t in enumerate(self.templates)])

        return OrderedDict(sorted(data_dict.items()))

    def encode(self) -> dict:
        data_dict: Dict[str, Union[str, list]] = {
            NAME.name: self.name,
            CROWDANKI_ID.name: self.crowdanki_id,
            CSS.name: self.css
        }

        SORT_FIELD_NUM.append_name_if_differs(data_dict, self.sort_field_num)
        IS_CLOZE.append_name_if_differs(data_dict, self.is_cloze)
        LATEX_PRE.append_name_if_differs(data_dict, self.latex_pre)
        LATEX_POST.append_name_if_differs(data_dict, self.latex_post)

        data_dict.setdefault(FIELDS.name, [f.encode_as_deck_part() for f in self.fields])
        data_dict.setdefault(TEMPLATES.name, [t.encode_as_deck_part() for t in self.templates])

        # Useless
        TAGS.append_name_if_differs(data_dict, self.tags)
        VERSION.append_name_if_differs(data_dict, self.version)
        CROWDANKI_TYPE.append_name_if_differs(data_dict, self.crowdanki_type)
        data_dict.setdefault(REQUIRED_FIELDS_PER_TEMPLATE.name, self.required_fields_per_template)

        return data_dict

    def get_all_media_references(self) -> set:
        all_media = set()
        for template in self.templates:
            all_media = all_media.union(template.get_all_media_references())

        return all_media

    @property
    def field_names_lowercase(self):
        return list_of_str_to_lowercase(f.name for f in self.fields)

    def check_field_overlap(self, fields_to_check: List[str]):
        fields_to_check = list_of_str_to_lowercase(fields_to_check)
        lower_fields = self.field_names_lowercase

        missing = [f for f in lower_fields if f not in fields_to_check]
        extra = [f for f in fields_to_check if f not in lower_fields]  # TODO: Remove?

        return missing, extra

    def zip_field_to_data(self, data: List[str]) -> dict:
        if len(self.fields) != len(data):
            raise Exception(f"Data of length {len(data)} cannot map to fields of length {len(self.field_names_lowercase)}")
        return dict(zip(self.field_names_lowercase, data))


