import logging
from dataclasses import dataclass, field
from typing import Set, Dict, List, Tuple

from brain_brew.interfaces.media_container import MediaContainer
from brain_brew.representation.generic.media_file import MediaFile
from brain_brew.representation.yaml.yaml_object import YamlObject
from brain_brew.utils import find_all_files_in_directory


@dataclass
class MediaGroup(YamlObject):
    media_files: Set[MediaFile]
    _media_filenames: Set[str] = field(default_factory=set())

    def encode(self) -> list:
        return list(self.media_files)

    @classmethod
    def from_directory(cls, directory: str, recursive: bool):
        files: Set[MediaFile] = set()
        for full_path in find_all_files_in_directory(directory, recursive=recursive):
            files.add(MediaFile.create_or_get(full_path))
        return cls(media_files=files)

    @classmethod
    def from_yaml_file(cls, filename: str):
        return cls(media_files=cls.from_list(cls.read_to_dict(filename)))

    def __post_init__(self):
        self._media_filenames = set(file.filename for file in self.media_files)

    @staticmethod
    def from_list(known_files: list):
        files: Set[MediaFile] = set()

        for full_path in known_files:
            if full_path not in files:
                if MediaFile.is_file(full_path):
                    files.add(MediaFile.create_or_get(full_path))
                else:
                    logging.error(f"Missing expected media file at '{full_path}'")
            else:
                logging.warning(f"Duplicate media file '{full_path}' in MediaGroup")

        return files

    def compare_media_containers(self, containers: List[MediaContainer]) -> Tuple[Set[MediaFile], Set[MediaFile], Set[str]]:
        all_to_compare: Set[str] = set.union(*[container.get_all_media_references() for container in containers])

        resolved, unresolved, missing = self.compare_filenames(all_to_compare)

        return resolved, unresolved, missing

    def compare_filenames(self, compare: Set[str]) -> Tuple[Set[MediaFile], Set[MediaFile], Set[str]]:
        resolved: Set[MediaFile] = set()
        unresolved: Set[MediaFile] = set()

        for media_file in self.media_files:
            if media_file.filename in compare:
                resolved.add(media_file)
            else:
                unresolved.add(media_file)

        missing: Set[str] = compare.difference(self._media_filenames)
        if len(missing) > 0:
            logging.error(f"Unresolved references in DeckParts to {len(missing)} files: {missing}")

        return resolved, unresolved, missing
