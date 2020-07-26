from typing import Dict, Type

from brain_brew.representation.build_config.build_task import TopLevelBuildTask, GenerateDeckPartBuildTask, BuildTask
from brain_brew.representation.build_config.task_builder import TaskBuilder


class GenerateDeckParts(TaskBuilder, TopLevelBuildTask):
    task_names = ["Generate Deck Parts", "Generate Deck Part", "Deck Part", "Deck Parts"]

    @classmethod
    def known_task_dict(cls) -> Dict[str, Type[BuildTask]]:
        return GenerateDeckPartBuildTask.get_all_build_tasks()
