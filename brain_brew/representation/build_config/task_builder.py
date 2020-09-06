from dataclasses import dataclass
from typing import Dict, List, Type
import re

from brain_brew.interfaces.verifiable import Verifiable
from brain_brew.representation.build_config.build_task import BuildTask
from brain_brew.representation.yaml.my_yaml import YamlRepr
from brain_brew.utils import str_to_lowercase_no_separators


@dataclass
class TaskBuilder(YamlRepr):
    tasks: List[BuildTask]

    @classmethod
    def from_list(cls, data: List[dict]):
        tasks = cls.read_tasks(data)
        return cls(
            tasks=tasks
        )

    @classmethod
    def known_task_dict(cls) -> Dict[str, Type[BuildTask]]:
        raise NotImplemented()

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
                task_instance = matching_task.from_repr(task_arguments)
                build_tasks.append(task_instance)
            else:
                raise KeyError(f"Unknown task '{task_name}'")  # TODO: check this first on all and return all errors

        # Verify tasks
        for task in build_tasks:
            if isinstance(task, Verifiable):
                task.verify_contents()

        return build_tasks

    def execute(self):
        for task in self.tasks:
            task.execute()

