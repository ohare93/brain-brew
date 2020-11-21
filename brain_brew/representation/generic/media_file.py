import shutil
import os
from dataclasses import dataclass, field

from brain_brew.representation.generic.source_file import SourceFile
from brain_brew.utils import filename_from_full_path


@dataclass
class MediaFile(SourceFile):
    file_path: str
    filename: str = field(init=False)

    def __post_init__(self):
        self.filename = filename_from_full_path(self.file_path)

    @classmethod
    def from_file_loc(cls, file_loc) -> 'MediaFile':
        return cls(file_loc)

    def __repr__(self):
        return f"MediaFile({self.file_path})"

    def __hash__(self):
        return hash(self.__repr__())

    def copy_self_to_target(self, target: str):
        shutil.copy2(self.file_path, target)

    def delete_self(self):
        os.remove(self.file_path)
