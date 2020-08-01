from dataclasses import dataclass, field
from typing import List, Optional


@dataclass
class CrowdAnkiNoteModel:
    @dataclass
    class Field:
        name: str
        ord: int
        font: str = field(default="Liberation Sans")
        media: List[str] = field(default_factory=lambda: [])
        rtl: bool = field(default=False)
        size: int = field(default=20)
        sticky: bool = field(default=False)

    @dataclass
    class Template:
        name: str
        ord: int
        afmt: str
        qfmt: str
        bafmt: str = field(default="")
        bqfmt: str = field(default="")
        did: Optional[int] = field(default=None)

    name: str
    crowdanki_uuid: str
    css: str
    flds: List[Field]
    tmpls: List[Template]
    latexPost: str
    latexPre: str
    req: List[list]
    sortf: int = field(default=0)
    tags: List[str] = field(default_factory=lambda: [])
    __type__: str = field(default="NoteModel")
    type: int = field(default=0)
    vers: list = field(default_factory=lambda: [])

    @classmethod
    def from_dict(cls, data):
        return cls(**data)

