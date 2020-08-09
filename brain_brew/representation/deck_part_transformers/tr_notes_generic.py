from dataclasses import dataclass, field
from typing import Optional
import re

from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.note_repr import Notes
from brain_brew.representation.configuration.global_config import GlobalConfig


@dataclass
class TrNotes:
    @staticmethod
    def split_tags(tags_value: str) -> list:
        split = [entry.strip() for entry in re.split(';\s*|,\s*|\s+', tags_value)]
        while "" in split:
            split.remove("")
        return split

    @staticmethod
    def join_tags(tags_list: list) -> str:
        return GlobalConfig.get_instance().flags.join_values_with.join(tags_list)


@dataclass
class TrGenericToNotes(TrNotes):
    @dataclass
    class Representation:
        name: str
        save_to_file: Optional[str]

        def __init__(self, name, save_to_file=None):
            self.name = name
            self.save_to_file = save_to_file

    name: str
    save_to_file: Optional[str]

    data: DeckPartHolder[Notes] = field(init=False)


@dataclass
class TrNotesToGeneric(TrNotes):
    @dataclass
    class Representation:
        notes: str

        def __init__(self, notes):
            self.notes = notes

    notes: DeckPartHolder[Notes]
