from typing import Dict, List, Type

from brain_brew.utils import str_to_lowercase_no_separators


class BuildTask(object):
    task_names: List[str]

    def execute(self):
        raise NotImplemented()

    @classmethod
    def from_dict(cls, data: dict):
        raise NotImplemented()

    @classmethod
    def get_all_build_tasks(cls) -> Dict[str, Type['BuildTask']]:
        subclasses: List[Type[BuildTask]] = cls.__subclasses__()
        known_build_tasks: Dict[str, Type[BuildTask]] = {}

        for sc in subclasses:
            for original_task_name in sc.task_names:
                task_name = str_to_lowercase_no_separators(original_task_name)

                if task_name in known_build_tasks:
                    raise KeyError(f"Multiple instances of task name '{task_name}'")
                elif task_name == "" or task_name is None:
                    raise KeyError(f"Unknown task name {original_task_name}")

                known_build_tasks.setdefault(task_name, sc)

        return known_build_tasks


class TopLevelBuildTask(BuildTask):
    pass


class DeckPartBuildTask(BuildTask):
    pass
