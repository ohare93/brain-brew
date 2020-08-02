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
MEDIA = "media"  # Unused in Anki
IS_RIGHT_TO_LEFT = "rtl"
FONT_SIZE = "size"
IS_STICKY = "sticky"

# Template
QUESTION_FORMAT = "qfmt"
ANSWER_FORMAT = "afmt"
BROWSER_ANSWER_FORMAT = "bafmt"
BROWSER_QUESTION_FORMAT = "bqfmt"
DECK_OVERRIDE_ID = "did"


@dataclass
class CrowdAnkiNoteModel:
    @dataclass
    class Field:
        @dataclass
        class Representation:
            name: str
            ord: int
            font: str = field(default="Liberation Sans")
            media: List[str] = field(default_factory=lambda: [])
            rtl: bool = field(default=False)
            size: int = field(default=20)
            sticky: bool = field(default=False)

            @classmethod
            def from_dict(cls, data: dict):
                return cls(**data)

        name: str
        ordinal: int
        font: str
        media: List[str]
        is_right_to_left: bool
        font_size: int
        is_sticky: bool

        @classmethod
        def from_repr(cls, data: Representation):
            return cls(
                name=data.name, ordinal=data.ord, font=data.font, media=data.media,
                is_right_to_left=data.rtl, font_size=data.size, is_sticky=data.sticky
            )

        @classmethod
        def from_dict(cls, data: dict):
            return cls.from_repr(cls.Representation.from_dict(data))

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
        question_format_in_browser: str
        answer_format_in_browser: str
        deck_override_id: Optional[int]

        @classmethod
        def from_repr(cls, data: Representation):
            return cls(
                name=data.name, ordinal=data.ord, question_format=data.qfmt, answer_format=data.afmt,
                question_format_in_browser=data.bqfmt, answer_format_in_browser=data.bafmt, deck_override_id=data.did
            )

        @classmethod
        def from_dict(cls, data: dict):
            return cls.from_repr(cls.Representation.from_dict(data))

    @dataclass
    class Representation:
        name: str
        crowdanki_uuid: str
        css: str
        latexPost: str
        latexPre: str
        req: List[list]
        flds: List[dict]
        tmpls: List[dict]
        __type__: str = field(default="NoteModel")
        tags: List[str] = field(default_factory=lambda: [])
        sortf: int = field(default=0)
        type: int = field(default=0)
        vers: list = field(default_factory=lambda: [])

        @classmethod
        def from_dict(cls, data: dict):
            return cls(**data)

    crowdanki_id: str
    crowdanki_type: str

    name: str
    css: str
    latex_post: str
    latex_pre: str
    required_fields_per_template: List[list]
    fields: List[Field]
    templates: List[Template]
    tags: List[str]
    sort_field_num: int
    is_cloze: bool
    version: list  # Deprecated in Anki

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

    # def encode(self) -> dict:
    #     data_dict = {}
    #     super().encode_groupable(data_dict)
    #     data_dict.setdefault(NOTES, [note.encode() for note in self.notes])
    #     return data_dict

    def find_media(self):
        pass
        # Look in templates (and css?)
