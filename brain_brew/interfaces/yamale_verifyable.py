from abc import ABC, abstractmethod


class YamlRepr(ABC):
    @classmethod
    @abstractmethod
    def task_name(cls) -> str:
        pass

    @classmethod
    @abstractmethod
    def yamale_schema(cls) -> str:
        pass

    @classmethod
    def yamale_dependencies(cls) -> set:
        return set()

    @classmethod
    @abstractmethod
    def from_repr(cls, data: dict):
        pass
