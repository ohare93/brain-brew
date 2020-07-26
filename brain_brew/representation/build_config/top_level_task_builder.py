from typing import Dict, Type

from brain_brew.representation.build_config.build_task import TopLevelBuildTask, BuildTask
from brain_brew.representation.build_config.task_builder import TaskBuilder


class TopLevelTaskBuilder(TaskBuilder):
    @classmethod
    def known_task_dict(cls) -> Dict[str, Type[BuildTask]]:
        return TopLevelBuildTask.get_all_build_tasks()
