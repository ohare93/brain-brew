from dataclasses import dataclass
from typing import Dict, Type, List

from brain_brew.representation.build_config.build_task import BuildTask, TopLevelBuildTask, DeckPartBuildTask
from brain_brew.representation.build_config.task_builder import TaskBuilder

# Build Tasks
from brain_brew.build_tasks.deck_parts.from_deck_part import FromDeckParts
from brain_brew.build_tasks.csvs.csvs_to_deck_parts import CsvsToDeckParts
from brain_brew.build_tasks.crowd_anki.crowd_anki_to_deck_parts import CrowdAnkiToDeckParts
from brain_brew.utils import str_to_lowercase_no_separators


@dataclass
class GenerateDeckParts(TaskBuilder, TopLevelBuildTask):
    task_regex = r'.*deck[\s_-]*?part.*'

    @classmethod
    def known_task_dict(cls) -> Dict[str, Type[BuildTask]]:
        return DeckPartBuildTask.get_all_task_regex()

    @classmethod
    def from_repr(cls, data: List[dict]):
        if not isinstance(data, list):
            raise TypeError(f"GenerateDeckParts needs a list")
        return cls.from_list(data)

