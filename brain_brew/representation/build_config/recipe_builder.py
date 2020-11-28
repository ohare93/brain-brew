import re
from abc import ABCMeta, abstractmethod
from dataclasses import dataclass
from typing import Dict, List, Type, Set
from textwrap import indent, dedent

from brain_brew.representation.build_config.build_task import BuildTask
from brain_brew.representation.yaml.yaml_object import YamlObject


@dataclass
class RecipeBuilder(YamlObject, metaclass=ABCMeta):
    tasks: List[BuildTask]

    @classmethod
    def from_list(cls, data: List[dict]):
        tasks = cls.read_tasks(data)
        return cls(
            tasks=tasks
        )

    @classmethod
    @abstractmethod
    def known_task_dict(cls) -> Dict[str, Type[BuildTask]]:
        pass

    @classmethod
    def build_yamale_root_node(cls, subclasses: Set[Type['BuildTask']]) -> str:
        task_list = [f"map(any(include('{c.task_name()}'), list(include('{c.task_name()}'))), key=regex('{c.task_regex()}', ignore_case=True))"
                     for c in sorted(subclasses, key=lambda x: x.task_name())]

        final_tasks: str = "list(\n" + indent(",\n".join(task_list), '  ') + "\n)\n"

        return final_tasks

    @classmethod
    def read_tasks(cls, tasks: List[dict]) -> list:
        task_regex_matches = cls.known_task_dict()
        build_tasks = []

        def find_matching_task(task_n):
            for regex, task_to_run in task_regex_matches.items():
                if re.match(regex, task_n, re.RegexFlag.IGNORECASE):
                    return task_to_run
            return None

        # Tasks
        for task in tasks:
            task_keys = list(task.keys())
            if len(task_keys) != 1:
                raise KeyError(f"Task should only contain 1 entry, but contains {task_keys} instead. "
                               f"Missing list separator '-'?", task)

            task_name = task_keys[0]
            task_arguments = task[task_keys[0]]

            matching_task = find_matching_task(task_name)
            if matching_task is not None:
                if isinstance(task_arguments, list):
                    task_or_tasks = [matching_task.from_repr(t_arg) for t_arg in task_arguments]
                else:
                    task_or_tasks = matching_task.from_repr(task_arguments)
                build_tasks.append(task_or_tasks)
            else:
                raise KeyError(f"Unknown task '{task_name}'")  # TODO: check this first on all and return all errors

        return build_tasks

    def execute(self):
        for task in self.tasks:
            task.execute()
