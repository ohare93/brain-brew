from dataclasses import dataclass, field
from enum import Enum

from brain_brew.constants.build_config_keys import BuildTaskEnum
from brain_brew.build_tasks.source_crowd_anki import SourceCrowdAnki
from brain_brew.build_tasks.source_csv import SourceCsv
from brain_brew.file_manager import FileManager
from brain_brew.interfaces.verifiable import Verifiable
from brain_brew.utils import single_item_to_list
from brain_brew.representation.configuration.global_config import GlobalConfig
from brain_brew.representation.generic.yaml_file import YamlFile, ConfigKey


@dataclass
class Builder:
    @dataclass
    class Representation:
        tasks: list

        @classmethod
        def from_dict(cls, data: dict):
            return cls(**data)

    tasks: list
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
        return cls.from_repr(Builder.Representation.from_dict(data), global_config, file_manager)

    @staticmethod
    def read_tasks(tasks: list) -> list:
        self.BUILD_TASK_DEFINITIONS = {build_task.key_name: build_task
                                       for source in self.KNOWN_BUILD_TASK_CLASSES
                                       for build_task in source.get_build_keys()
                                       }


        build_tasks = []

        # Tasks
        for key in self.config_entry[BuilderKeys.TASKS.value]:
            tasks = single_item_to_list(key)
            for task in tasks:
                task_keys = list(task.keys())
                if len(task_keys) != 1:
                    raise KeyError(f"Task should only contain 1 entry, but contains {task_keys} instead", task)

                if task_keys[0] in self.BUILD_TASK_DEFINITIONS.keys():
                    definition: BuildTaskEnum = self.BUILD_TASK_DEFINITIONS[task_keys[0]]
                    source = definition.source_type(task[task_keys[0]], read_now)
                    if self.reverse_run_direction:
                        build_tasks.append((source, definition.reverse_task_to_execute))
                    else:
                        build_tasks.append((source, definition.task_to_execute))
                else:
                    raise KeyError(f"Unknown key {key}")  # TODO: check this first on all and return all errors

        if self.reverse_run_direction:
            build_tasks = list(reversed(build_tasks))

        # Verify tasks
        for source, task_to_execute in build_tasks:
            if isinstance(source, Verifiable):
                source.verify_contents()

        return build_tasks

    def execute(self):
        for (source, task_to_execute) in self.tasks:
            getattr(source, task_to_execute)()

        self.file_manager.write_to_all()
