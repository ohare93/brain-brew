from dataclasses import dataclass

from brain_brew.interfaces.yamale_verifyable import YamlRepr


@dataclass
class MediaGroupsToFolder(YamlRepr):
    @classmethod
    def task_regex(cls) -> str:
        return r"media_group_to_folder"

    @classmethod
    def yamale_validator_and_deps(cls) -> (str, set):
        pass

    @classmethod
    def from_repr(cls, data: dict):
        pass