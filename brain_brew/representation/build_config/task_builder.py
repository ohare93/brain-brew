from dataclasses import dataclass
from typing import Dict, List, Type

from brain_brew.file_manager import FileManager
from brain_brew.interfaces.verifiable import Verifiable
from brain_brew.representation.build_config.build_task import BuildTask
from brain_brew.representation.configuration.global_config import GlobalConfig
from brain_brew.utils import str_to_lowercase_no_separators


@dataclass
class TaskBuilder:
    @dataclass
    class Representation:
        tasks: list

        @classmethod
        def from_dict(cls, data: dict):
            return cls(**data)

    tasks: List[BuildTask]
    global_config: GlobalConfig
    file_manager: FileManager

    @classmethod
    def from_repr(cls, data: Representation, global_config, file_manager):
        tasks = cls.read_tasks(data.tasks)
        return cls(
            tasks=tasks,
            global_config=global_config,
            file_manager=file_manager
        )

    @classmethod
    def from_dict(cls, data: dict, global_config, file_manager):
        return cls.from_repr(TaskBuilder.Representation.from_dict(data), global_config, file_manager)

    @classmethod
    def known_task_dict(cls) -> Dict[str, Type[BuildTask]]:
        raise NotImplemented()

    @classmethod
    def read_tasks(cls, tasks: list) -> list:
        known_task_dict = cls.known_task_dict()
        build_tasks = []

        # Tasks
        for task in tasks:
            task_keys = list(task.keys())
            if len(task_keys) != 1:
                raise KeyError(f"Task should only contain 1 entry, but contains {task_keys} instead. "
                               f"Missing list separator '-'?", task)

            task_name = str_to_lowercase_no_separators(task_keys[0])
            task_arguments = task[task_keys[0]]
            if task_name in known_task_dict:
                task_instance = known_task_dict[task_name].from_dict(task_arguments)
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

        self.file_manager.write_to_all()
