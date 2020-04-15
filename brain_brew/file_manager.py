import glob
import logging
import re
from typing import Dict

from brain_brew.representation.configuration.global_config import GlobalConfig
from brain_brew.representation.generic.generic_file import GenericFile
from brain_brew.representation.media.deck_part_media_file import DeckPartMediaFile


class FileManager:
    __instance = None
    global_config: GlobalConfig

    known_files_dict: Dict[str, GenericFile]
    known_media_files_dict: Dict[str, DeckPartMediaFile]

    def __init__(self):
        if FileManager.__instance is None:
            FileManager.__instance = self
        else:
            raise Exception("Multiple FileManagers created")

        self.global_config = GlobalConfig.get_instance()

        self.known_files_dict = {}

        self.find_all_media_files()

    @staticmethod
    def get_instance():
        return FileManager.__instance

    @staticmethod
    def clear_instance():
        if FileManager.__instance:
            FileManager.__instance = None

    def file_if_exists(self, file_location) -> GenericFile:
        if file_location in self.known_files_dict.keys():
            return self.known_files_dict[file_location]
        return None

    def media_file_if_exists(self, file_location) -> DeckPartMediaFile:
        if file_location in self.known_media_files_dict.keys():
            return self.known_media_files_dict[file_location]
        return None

    def register_file(self, file_location, file):
        if file_location in self.known_files_dict:
            raise FileExistsError("File already known to FileManager, cannot be registered twice")
        self.known_files_dict.setdefault(file_location, file)

    def find_all_media_files(self):
        self.known_media_files_dict = {}

        for full_path in glob.iglob(self.global_config.deck_parts.media_files + '**/*.*', recursive=True):
            filename = re.findall('[^\\/:*?"<>|\r\n]+$', full_path)[0]
            if filename not in self.known_media_files_dict:
                self.known_media_files_dict.setdefault(filename, DeckPartMediaFile(full_path, filename))
            else:
                logging.error(f"Duplicate media file '{filename}' in both '{full_path}'"
                              f" and '{self.known_media_files_dict[filename]}'")

        logging.debug(f"Media files found: {len(self.known_media_files_dict)}")

    def write_to_all(self):
        files_to_create = []
        for location, file in self.known_files_dict.items():
            if not file.file_exists:
                files_to_create.append(location)

        # logging.info(f"Will create {len(files_to_create)} new files: ", files_to_create)

        for location, file in self.known_files_dict.items():
            if file.data_state == GenericFile.DataState.DATA_SET:
                logging.info(f"Wrote to {file.file_location}")
                file.write_file()
                file.data_state = GenericFile.DataState.READ_IN_DATA

