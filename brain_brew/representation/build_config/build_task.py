import logging
from abc import ABCMeta, abstractmethod
from typing import Dict, List, Type, Tuple

from brain_brew.interfaces.yamale_verifyable import YamlRepr


class BuildTask(YamlRepr, object, metaclass=ABCMeta):
    @abstractmethod
    def execute(self):
        pass

    @classmethod
    def get_all_task_regex(cls) -> Dict[str, Type['BuildTask']]:
        subclasses: List[Type[BuildTask]] = cls.__subclasses__()
        task_regex_matches: Dict[str, Type[BuildTask]] = {}

        for sc in subclasses:
            if sc.task_regex in task_regex_matches:
                raise KeyError(f"Multiple instances of task regex '{sc.task_regex}'")
            elif sc.task_regex == "" or sc.task_regex is None:
                raise KeyError(f"Unknown task regex {sc.task_regex}")

            task_regex_matches.setdefault(sc.task_regex(), sc)

        # logging.debug(f"Known build tasks: {known_build_tasks}")
        return task_regex_matches

    @classmethod
    def get_all_validators(cls) -> List[Tuple[str, set]]:
        subclasses: List[Type[BuildTask]] = cls.__subclasses__()
        return [sc.yamale_validator_and_deps() for sc in subclasses]


class TopLevelBuildTask(BuildTask, metaclass=ABCMeta):
    @classmethod
    def get_all_task_regex(cls) -> Dict[str, Type['BuildTask']]:
        return super(TopLevelBuildTask, cls).get_all_task_regex()

    @classmethod
    def get_all_validators(cls) -> List[Tuple[str, set]]:
        return super(TopLevelBuildTask, cls).get_all_validators()


class PartBuildTask(BuildTask, metaclass=ABCMeta):
    @classmethod
    def get_all_task_regex(cls) -> Dict[str, Type['BuildTask']]:
        return super(PartBuildTask, cls).get_all_task_regex()

    @classmethod
    def get_all_validators(cls) -> List[Tuple[str, set]]:
        return super(PartBuildTask, cls).get_all_validators()
