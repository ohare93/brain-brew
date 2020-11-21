from dataclasses import dataclass
from typing import Set, Dict, List, Tuple

from brain_brew.representation.generic.media_file import MediaFile
from brain_brew.representation.yaml.yaml_object import YamlObject
from brain_brew.utils import find_all_files_in_directory


@dataclass
class MediaGroup(YamlObject):
    media_files: Dict[str, MediaFile]

    def encode(self) -> list:
        return list(m.file_path for m in self.media_files.values())  # TODO: Use relative path for directory?

    @classmethod
    def from_yaml_file(cls, filename: str) -> 'MediaGroup':
        return cls(media_files=cls.from_full_path_list(cls.read_to_dict(filename)))

    @classmethod
    def from_directory(cls, directory: str, recursive: bool) -> 'MediaGroup':
        return cls(media_files=cls.from_full_path_list(find_all_files_in_directory(directory, recursive=recursive)))

    @classmethod
    def from_many(cls, groups: List['MediaGroup']) -> 'MediaGroup':
        files = list(set(file.file_path for group in groups for file in group.media_files.values()))
        return cls(media_files=cls.from_full_path_list(files))

    @staticmethod
    def from_full_path_list(known_files: list):
        files: Dict[str, MediaFile] = dict()

        for full_path in known_files:
            file = MediaFile.create_or_get(full_path)
            if file.filename not in files.keys():
                files[file.filename] = file
            else:
                raise NameError(f"Duplicate files with same filename '{file.filename}' in group")

        return files

    def remove_by_filename(self, filename: str):
        self.media_files.pop(filename, None)

    def filter_by_filenames(self, filenames: List[str], should_match: bool):
        for media_filename in self.media_files.keys():
            is_match = media_filename in filenames
            if is_match != should_match:
                self.remove_by_filename(media_filename)
        # TODO: Find all missing files

    def compare(self, other: 'MediaGroup') -> Tuple[Set[str], Set[str], Set[str]]:
        intersection = set(self.media_files.keys() & other.media_files.keys())
        difference_main = set(self.media_files.keys() ^ other.media_files.keys())
        difference_other = set(self.media_files.keys() ^ other.media_files.keys())

        return intersection, difference_main, difference_other
