from dataclasses import dataclass
from typing import Optional


@dataclass
class TrGenericToDeckPart:
    @dataclass
    class Representation:
        name: str
        save_to_file: Optional[str]

        def __init__(self, name, save_to_file=None):
            self.name = name
            self.save_to_file = save_to_file

        @classmethod
        def from_dict(cls, data: dict):
            return cls(**data)

    name: str
    save_to_file: Optional[str]
