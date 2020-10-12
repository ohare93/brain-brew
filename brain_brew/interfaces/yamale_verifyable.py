from abc import ABC, abstractmethod


class YamlRepr(ABC):
    @classmethod
    @abstractmethod
    def task_regex(cls) -> str:
        pass

    @classmethod
    @abstractmethod
    def yamale_validator_and_deps(cls) -> (str, set):
        pass

    @classmethod
    @abstractmethod
    def from_repr(cls, data: dict):
        pass
