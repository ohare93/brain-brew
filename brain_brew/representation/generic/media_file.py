import shutil
from enum import Enum


class MediaFile:
    class ManagementType(Enum):
        EXISTS = 0
        OVERRIDDEN = 1
        TO_BE_CLONED = 2

    target_loc: str
    filename: str

    man_type: ManagementType
    source_loc: str

    def __init__(self, target_loc, filename, man_type: ManagementType = ManagementType.EXISTS, source_loc=None):
        self.target_loc = target_loc
        self.filename = filename

        self.man_type = man_type
        self.source_loc = target_loc if source_loc is None else source_loc

    def set_override(self, source_loc):
        self.man_type = MediaFile.ManagementType.OVERRIDDEN
        self.source_loc = source_loc

    def copy_source_to_target(self):
        if self.should_write():
            # TODO: If ManagementType.OVERRIDDEN check if override necessary
            shutil.copy2(self.source_loc, self.target_loc)

    def should_write(self):
        return self.man_type in [MediaFile.ManagementType.OVERRIDDEN, MediaFile.ManagementType.TO_BE_CLONED]
