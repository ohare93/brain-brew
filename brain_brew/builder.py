import logging
from enum import Enum

from brain_brew.constants.build_config_keys import BuildTaskEnum
from brain_brew.build_tasks.source_crowd_anki import SourceCrowdAnki
from brain_brew.build_tasks.source_csv import SourceCsv
from brain_brew.build_tasks.source_csv_collection import SourceCsvCollection
from brain_brew.file_manager import FileManager
from brain_brew.utils import single_item_to_list
from brain_brew.representation.configuration.global_config import GlobalConfig
from brain_brew.representation.configuration.yaml_file import YamlFile, ConfigKey


class BuilderKeys(Enum):
    TASKS = "tasks"
    REVERSE_RUN_DIRECTION = "reverse"


class Builder(YamlFile):
    config_entry = {}
    expected_keys = {
        BuilderKeys.TASKS.value: ConfigKey(True, list, None),
        BuilderKeys.REVERSE_RUN_DIRECTION.value: ConfigKey(False, bool, None)
    }
    subconfig_filter = None

    global_config: GlobalConfig

    BUILD_TASK_DEFINITIONS: dict
    KNOWN_BUILD_TASK_CLASSES = [SourceCrowdAnki, SourceCsv, SourceCsvCollection]

    build_tasks = []
    file_manager: FileManager

    def __init__(self, config_data, global_config, other_args, read_now=True):
        self.file_manager = FileManager.get_instance()

        self.BUILD_TASK_DEFINITIONS = {build_task.key_name: build_task
                                       for source in self.KNOWN_BUILD_TASK_CLASSES
                                       for build_task in source.get_build_keys()
                                       }

        self.setup_config_with_subconfig_replacement(config_data)
        self.verify_config_entry()

        self.global_config = global_config

        self.reverse_run_direction = self.get_config(BuilderKeys.REVERSE_RUN_DIRECTION, False)

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
                        self.build_tasks.append((source, definition.reverse_task_to_execute))
                    else:
                        self.build_tasks.append((source, definition.task_to_execute))
                else:
                    raise KeyError(f"Unknown key {key}")  # TODO: check this first on all and return all errors

        if self.reverse_run_direction:
            self.build_tasks = list(reversed(self.build_tasks))

    def execute(self):
        for (source, task_to_execute) in self.build_tasks:
            getattr(source, task_to_execute)()

        self.file_manager.write_to_all()
