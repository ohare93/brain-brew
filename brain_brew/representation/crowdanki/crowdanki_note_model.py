from collections import OrderedDict
from dataclasses import dataclass, field
from typing import List, Optional

# CrowdAnki
CROWDANKI_ID = "crowdanki_uuid"
CROWDANKI_TYPE = "__type__"

# Shared
NAME = "name"
ORDINAL = "ord"

# Note Model
CSS = "css"
LATEX_POST = "latexPost"
LATEX_PRE = "latexPre"
REQUIRED_FIELDS_PER_TEMPLATE = "req"
FIELDS = "flds"
TEMPLATES = "tmpls"
TAGS = "tags"
SORT_FIELD_NUM = "sortf"
IS_CLOZE = "type"
VERSION = "vers"

# Field
FONT = "font"
MEDIA = "media"
IS_RIGHT_TO_LEFT = "rtl"
FONT_SIZE = "size"
IS_STICKY = "sticky"

# Template
QUESTION_FORMAT = "qfmt"
ANSWER_FORMAT = "afmt"
BROWSER_ANSWER_FORMAT = "bafmt"
BROWSER_QUESTION_FORMAT = "bqfmt"
DECK_OVERRIDE_ID = "did"


# Defaults
DEFAULT_LATEX_PRE = "\\documentclass[12pt]{article}\n\\special{papersize=3in,5in}\n\\usepackage{amssymb,amsmath}\n\\pagestyle{empty}\n\\setlength{\\parindent}{0in}\n\\begin{document}\n"
DEFAULT_LATEX_POST = "\\end{document}"
DEFAULT_CROWDANKI_TYPE = "NoteModel"
DEFAULT_FONT = "Liberation Sans"


@dataclass
class CrowdAnkiNoteModel:
    @dataclass
    class Field:
        @dataclass
        class Representation:
            name: str
            ord: int
            font: str = field(default=DEFAULT_FONT)
            media: List[str] = field(default_factory=lambda: [])
            rtl: bool = field(default=False)
            size: int = field(default=20)
            sticky: bool = field(default=False)

            @classmethod
            def from_dict(cls, data: dict):
                return cls(**data)

        name: str
        ordinal: int
        font: str = field(default=DEFAULT_FONT)
        media: List[str] = field(default_factory=lambda: [])  # Unused in Anki
        is_right_to_left: bool = field(default=False)
        font_size: int = field(default=20)
        is_sticky: bool = field(default=False)

        @classmethod
        def from_repr(cls, data: Representation):
            return cls(
                name=data.name, ordinal=data.ord, font=data.font, media=data.media,
                is_right_to_left=data.rtl, font_size=data.size, is_sticky=data.sticky
            )

        @classmethod
        def from_dict(cls, data: dict):
            return cls.from_repr(cls.Representation.from_dict(data))

        def encode(self) -> dict:
            data_dict = {
                NAME: self.name,
                ORDINAL: self.ordinal,
                FONT: self.font,
                MEDIA: self.media,
                IS_RIGHT_TO_LEFT: self.is_right_to_left,
                FONT_SIZE: self.font_size,
                IS_STICKY: self.is_sticky
            }

            return data_dict

    @dataclass
    class Template:
        @dataclass
        class Representation:
            name: str
            ord: int
            qfmt: str
            afmt: str
            bqfmt: str = field(default="")
            bafmt: str = field(default="")
            did: Optional[int] = field(default=None)

            @classmethod
            def from_dict(cls, data: dict):
                return cls(**data)

        name: str
        ordinal: int
        question_format: str
        answer_format: str
        question_format_in_browser: str = field(default="")
        answer_format_in_browser: str = field(default="")
        deck_override_id: Optional[int] = field(default=None)

        @classmethod
        def from_repr(cls, data: Representation):
            return cls(
                name=data.name, ordinal=data.ord, question_format=data.qfmt, answer_format=data.afmt,
                question_format_in_browser=data.bqfmt, answer_format_in_browser=data.bafmt, deck_override_id=data.did
            )

        @classmethod
        def from_dict(cls, data: dict):
            return cls.from_repr(cls.Representation.from_dict(data))

        def encode(self) -> dict:
            data_dict = {
                NAME: self.name,
                ORDINAL: self.ordinal,
                QUESTION_FORMAT: self.question_format,
                ANSWER_FORMAT: self.answer_format,
                BROWSER_QUESTION_FORMAT: self.question_format_in_browser,
                BROWSER_ANSWER_FORMAT: self.answer_format_in_browser,
                DECK_OVERRIDE_ID: self.deck_override_id
            }

            return data_dict

    @dataclass
    class Representation:
        name: str
        crowdanki_uuid: str
        css: str
        req: List[list]
        flds: List[dict]
        tmpls: List[dict]
        latexPre: str = field(default=DEFAULT_LATEX_PRE)
        latexPost: str = field(default=DEFAULT_LATEX_POST)
        __type__: str = field(default=DEFAULT_CROWDANKI_TYPE)
        tags: List[str] = field(default_factory=lambda: [])
        sortf: int = field(default=0)
        type: int = field(default=0)
        vers: list = field(default_factory=lambda: [])

        @classmethod
        def from_dict(cls, data: dict):
            return cls(**data)

    name: str
    crowdanki_id: str
    css: str
    required_fields_per_template: List[list]
    fields: List[Field]
    templates: List[Template]

    latex_post: str = field(default=DEFAULT_LATEX_PRE)
    latex_pre: str = field(default=DEFAULT_LATEX_POST)
    sort_field_num: int = field(default=0)
    is_cloze: bool = field(default=False)
    crowdanki_type: str = field(default=DEFAULT_CROWDANKI_TYPE)  # Should always be "NoteModel"
    tags: List[str] = field(default_factory=lambda: [])  # Tags of the last added note
    version: list = field(default_factory=lambda: [])  # Legacy version number. Deprecated in Anki

    @classmethod
    def from_repr(cls, data: Representation):
        return cls(
            fields=[CrowdAnkiNoteModel.Field.from_dict(f) for f in data.flds],
            templates=[CrowdAnkiNoteModel.Template.from_dict(t) for t in data.tmpls],
            is_cloze=bool(data.type),
            name=data.name, css=data.css, latex_pre=data.latexPre, latex_post=data.latexPost,
            required_fields_per_template=data.req, tags=data.tags, sort_field_num=data.sortf, version=data.vers,
            crowdanki_id=data.crowdanki_uuid, crowdanki_type=data.__type__
        )

    @classmethod
    def from_dict(cls, data: dict):
        return cls.from_repr(cls.Representation.from_dict(data))

    def encode(self) -> dict:
        data_dict = {
            NAME: self.name,
            CROWDANKI_ID: self.crowdanki_id,
            CSS: self.css,
            REQUIRED_FIELDS_PER_TEMPLATE: self.required_fields_per_template,
            LATEX_PRE: self.latex_pre,
            LATEX_POST: self.latex_post,
            SORT_FIELD_NUM: self.sort_field_num,
            CROWDANKI_TYPE: self.crowdanki_type,
            TAGS: self.tags,
            VERSION: self.version,
            IS_CLOZE: 1 if self.is_cloze else 0
        }

        data_dict.setdefault(FIELDS, [f.encode() for f in self.fields])
        data_dict.setdefault(TEMPLATES, [t.encode() for t in self.templates])

        return OrderedDict(sorted(data_dict.items()))

    def find_media(self):
        pass
        # Look in templates (and css?)
