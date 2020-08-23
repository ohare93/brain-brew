import glob
import logging
import pathlib
from typing import Dict, List, Union


from brain_brew.representation.configuration.global_config import GlobalConfig
from brain_brew.representation.generic.generic_file import SourceFile
from brain_brew.representation.generic.media_file import MediaFile
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.my_yaml import YamlRepr
from brain_brew.utils import filename_from_full_path, find_all_files_in_directory


class FileManager:
    __instance = None
    global_config: GlobalConfig

    known_files_dict: Dict[str, SourceFile]
    known_media_files_dict: Dict[str, MediaFile]
    deck_part_pool: Dict[str, DeckPartHolder[YamlRepr]]

    def __init__(self):
        if FileManager.__instance is None:
            FileManager.__instance = self
        else:
            raise Exception("Multiple FileManagers created")

        self.global_config = GlobalConfig.get_instance()

        self.known_files_dict = {}
        self.deck_part_pool = {}

        self.find_all_deck_part_media_files()

    @staticmethod
    def get_instance() -> 'FileManager':
        return FileManager.__instance

    @staticmethod
    def clear_instance():
        if FileManager.__instance:
            FileManager.__instance = None

    def file_if_exists(self, file_location) -> Union[SourceFile, None]:
        if file_location in self.known_files_dict.keys():
            return self.known_files_dict[file_location]
        return None

    def deck_part_if_exists(self, dp_name) -> Union[DeckPartHolder[YamlRepr], None]:
        return self.deck_part_pool.get(dp_name)

    def register_file(self, full_path, file):
        if full_path in self.known_files_dict:
            raise FileExistsError(f"File already known to FileManager, cannot be registered twice: {full_path}")
        self.known_files_dict.setdefault(full_path, file)

    def media_file_if_exists(self, filename) -> Union[MediaFile, None]:
        if filename in self.known_media_files_dict.keys():
            return self.known_media_files_dict[filename]
        return None
    
    def _register_media_file(self, file: MediaFile):
        if file.filename not in self.known_media_files_dict:
            self.known_media_files_dict.setdefault(file.filename, file)
        else:
            logging.error(f"Duplicate media file '{file.filename}' in both '{file.source_loc}'"
                          f" and '{self.known_media_files_dict[file.filename].source_loc}'")

    def new_media_file(self, filename, source_loc):
        self._register_media_file(MediaFile(self.global_config.deck_parts.media_files + filename,
                                            filename, MediaFile.ManagementType.TO_BE_CLONED, source_loc))

    def find_all_deck_part_media_files(self):
        self.known_media_files_dict = {}

        for full_path in find_all_files_in_directory(self.global_config.deck_parts.media_files, recursive=True):
            filename = filename_from_full_path(full_path)
            self._register_media_file(MediaFile(full_path, filename))

        logging.debug(f"DeckPart Media files found: {len(self.known_media_files_dict)}")

    def new_deck_part(self, dp: DeckPartHolder) -> DeckPartHolder:
        if dp.name in self.deck_part_pool:
            raise KeyError(f"Cannot use same name '{dp.name}' for multiple Deck Parts")
        self.deck_part_pool.setdefault(dp.name, dp)
        return dp

    def deck_part_from_pool(self, name: str):
        if name not in self.deck_part_pool:
            raise KeyError(f"Cannot find Deck Part '{name}'")
        return self.deck_part_pool[name]

    def write_to_all(self):
        # logging.info(f"Will create {len(files_to_create)} new files: ", files_to_create)

        for filename, media_file in self.known_media_files_dict.items():
            media_file.copy_source_to_target()
