from abc import ABCMeta, abstractmethod
from typing import Dict, Type, Set, Optional

from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.interfaces.yamale_verifyable import YamlRepr


class BuildTask(YamlRepr, object, metaclass=ABCMeta):
    execute_immediately: bool = False
    accepts_list_of_self: bool = True
    rep: Optional[RepresentationBase]

    def encode_rep(self) -> Dict[str, any]:
        return self.rep.encode()

    @abstractmethod
    def execute(self):
        pass

    @classmethod
    def task_regex(cls) -> str:
        return cls.task_name()

    @classmethod
    def get_all_task_regex(cls, subclasses: Set[Type['BuildTask']]) -> Dict[str, Type['BuildTask']]:
        task_regex_matches: Dict[str, Type[BuildTask]] = {}

        for sc in subclasses:
            if sc.task_regex in task_regex_matches:
                raise KeyError(f"Multiple instances of task regex '{sc.task_regex}'")
            elif sc.task_regex == "" or sc.task_regex is None:
                raise KeyError(f"Unknown task regex in {sc.__name__}")

            task_regex_matches.setdefault(sc.task_regex(), sc)

        # logging.debug(f"Known build tasks: {known_build_tasks}")
        return task_regex_matches


class TopLevelBuildTask(BuildTask, metaclass=ABCMeta):
    pass


class BuildPartTask(BuildTask, metaclass=ABCMeta):
    execute_immediately: bool = True
