import logging
from typing import Dict, Type

from brain_brew.representation.build_config.build_task import BuildTask, TopLevelBuildTask
from brain_brew.representation.build_config.task_builder import TaskBuilder

# Build Tasks
from brain_brew.build_tasks.csvs.csvs_generate import CsvsGenerate
from brain_brew.build_tasks.crowd_anki.crowd_anki_generate import CrowdAnkiGenerate
from brain_brew.representation.build_config.generate_deck_parts import GenerateDeckParts


class TopLevelTaskBuilder(TaskBuilder):
    @classmethod
    def known_task_dict(cls) -> Dict[str, Type[BuildTask]]:
        return TopLevelBuildTask.get_all_build_tasks()
