from collections import OrderedDict
from dataclasses import dataclass, field
from typing import List, Union, Dict, Set

from brain_brew.configuration.anki_field import AnkiField
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.interfaces.media_container import MediaContainer
from brain_brew.representation.yaml.note_model_field import Field
from brain_brew.representation.yaml.note_model_template import Template
from brain_brew.representation.yaml.yaml_object import YamlObject
from brain_brew.utils import list_of_str_to_lowercase

# CrowdAnki
CROWDANKI_ID = AnkiField("crowdanki_uuid", "id")
CROWDANKI_TYPE = AnkiField("__type__", default_value="NoteModel")

# Shared
NAME = AnkiField("name")
ORDINAL = AnkiField("ord", "ordinal")

# Note Model
CSS = AnkiField("css")
LATEX_PRE = AnkiField("latexPre", "latex_pre",
                      default_value="\\documentclass[12pt]{article}\n\\special{papersize=3in,5in}\n\\usepackage{"
                                    "amssymb,amsmath}\n\\pagestyle{empty}\n\\setlength{\\parindent}{0in}\n\\begin{"
                                    "document}\n")
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
FONT_SIZE = AnkiField("size", "font_size", default_value=20)
IS_STICKY = AnkiField("sticky", "is_sticky", default_value=False)

# Template
QUESTION_FORMAT = AnkiField("qfmt", "question_format")
ANSWER_FORMAT = AnkiField("afmt", "answer_format")
BROWSER_ANSWER_FORMAT = AnkiField("bafmt", "browser_answer_format", default_value="")
BROWSER_QUESTION_FORMAT = AnkiField("bqfmt", "browser_question_format", default_value="")
DECK_OVERRIDE_ID = AnkiField("did", "deck_override_id", default_value=None)


@dataclass
class NoteModel(YamlObject, MediaContainer, RepresentationBase):
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
    id: str
    css: str
    required_fields_per_template: List[list]
    fields: List[Field]
    templates: List[Template]

    latex_post: str = field(default=LATEX_POST.default_value)
    latex_pre: str = field(default=LATEX_PRE.default_value)
    sort_field_num: int = field(default=SORT_FIELD_NUM.default_value)
    is_cloze: bool = field(default=IS_CLOZE.default_value)
    crowdanki_type: str = field(default=CROWDANKI_TYPE.default_value)  # Should always be "NoteModel"
    tags: List[str] = field(default_factory=lambda: TAGS.default_value)  # Tags of the last added note
    version: list = field(default_factory=lambda: VERSION.default_value)  # Legacy version number. Deprecated in Anki

    @classmethod
    def from_yaml_file(cls, filename: str):
        data = cls.read_to_dict(filename)
        return cls(
            fields=[Field.from_dict(f) for f in data.pop(FIELDS.name)],
            templates=[Template.from_dict(t) for t in data.pop(TEMPLATES.name)],
            **data
        )

    @classmethod
    def from_crowdanki(cls, data: Union[CrowdAnki, dict]):  # TODO: field_whitelist, note_model_whitelist
        ca: cls.CrowdAnki = data if isinstance(data, cls.CrowdAnki) else cls.CrowdAnki.from_dict(data)
        return cls(
            fields=[Field.from_crowd_anki(f) for f in ca.flds],
            templates=[Template.from_crowdanki(t) for t in ca.tmpls],
            is_cloze=bool(ca.type),
            name=ca.name, css=ca.css, latex_pre=ca.latexPre, latex_post=ca.latexPost,
            required_fields_per_template=ca.req, tags=ca.tags, sort_field_num=ca.sortf, version=ca.vers,
            id=ca.crowdanki_uuid, crowdanki_type=ca.__type__
        )

    def encode_as_crowdanki(self) -> dict:
        data_dict = {
            NAME.anki_name: self.name,
            CROWDANKI_ID.anki_name: self.id,
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
            CROWDANKI_ID.name: self.id,
            CSS.name: self.css
        }

        SORT_FIELD_NUM.append_name_if_differs(data_dict, self.sort_field_num)
        IS_CLOZE.append_name_if_differs(data_dict, self.is_cloze)
        LATEX_PRE.append_name_if_differs(data_dict, self.latex_pre)
        LATEX_POST.append_name_if_differs(data_dict, self.latex_post)

        data_dict.setdefault(FIELDS.name, [f.encode_as_part() for f in self.fields])
        data_dict.setdefault(TEMPLATES.name, [t.encode() for t in self.templates])

        # Useless
        TAGS.append_name_if_differs(data_dict, self.tags)
        VERSION.append_name_if_differs(data_dict, self.version)
        CROWDANKI_TYPE.append_name_if_differs(data_dict, self.crowdanki_type)
        data_dict.setdefault(REQUIRED_FIELDS_PER_TEMPLATE.name, self.required_fields_per_template)

        return data_dict

    def get_all_media_references(self) -> Set[str]:
        all_media = set()
        for template in self.templates:
            all_media = all_media.union(template.get_all_media_references())

        return all_media

    @property
    def field_names_lowercase(self):
        return list_of_str_to_lowercase([f.name for f in self.fields])

    def check_field_overlap(self, fields_to_check: List[str]):
        fields_to_check = list_of_str_to_lowercase(fields_to_check)

        missing = [f for f in self.field_names_lowercase if f not in fields_to_check]

        return missing

    def check_field_extra(self, fields_to_check: List[str]):
        fields_to_check = list_of_str_to_lowercase(fields_to_check)

        return [f for f in fields_to_check if f not in self.field_names_lowercase]

    def zip_field_to_data(self, data: List[str]) -> dict:
        if len(self.fields) != len(data):
            raise Exception(
                f"Data of length {len(data)} cannot map to fields of length {len(self.field_names_lowercase)}")
        return dict(zip(self.field_names_lowercase, data))


