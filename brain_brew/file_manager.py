import glob
import logging
import pathlib
from typing import Dict, List

from brain_brew.interfaces.writes_file import WritesFile
from brain_brew.representation.configuration.global_config import GlobalConfig
from brain_brew.representation.generic.generic_file import GenericFile
from brain_brew.representation.generic.media_file import MediaFile
from brain_brew.utils import filename_from_full_path, find_all_files_in_directory


class FileManager:
    __instance = None
    global_config: GlobalConfig

    known_files_dict: Dict[str, GenericFile]
    known_media_files_dict: Dict[str, MediaFile]

    write_files_at_end: List[WritesFile]

    def __init__(self):
        if FileManager.__instance is None:
            FileManager.__instance = self
        else:
            raise Exception("Multiple FileManagers created")

        self.global_config = GlobalConfig.get_instance()

        self.known_files_dict = {}
        self.write_files_at_end = []

        self.find_all_deck_part_media_files()

    @staticmethod
    def get_instance() -> 'FileManager':
        return FileManager.__instance

    @staticmethod
    def clear_instance():
        if FileManager.__instance:
            FileManager.__instance = None

    def file_if_exists(self, file_location) -> GenericFile:
        if file_location in self.known_files_dict.keys():
            return self.known_files_dict[file_location]
        return None

    def register_file(self, full_path, file):
        if full_path in self.known_files_dict:
            raise FileExistsError("File already known to FileManager, cannot be registered twice")
        self.known_files_dict.setdefault(full_path, file)

    def media_file_if_exists(self, filename) -> MediaFile:
        if filename in self.known_media_files_dict.keys():
            return self.known_media_files_dict[filename]
        return None
    
    def _register_media_file(self, file: MediaFile):
        if file.filename not in self.known_media_files_dict:
            self.known_media_files_dict.setdefault(file.filename, file)
        else:
            logging.error(f"Duplicate media file '{file.filename}' in both '{file.source_loc}'"
                          f" and '{self.known_media_files_dict[file.filename].source_loc}'")

    def register_write_file_for_end(self, file: WritesFile):
        self.write_files_at_end.append(file)

    def new_media_file(self, filename, source_loc):
        self._register_media_file(MediaFile(self.global_config.deck_parts.media_files + filename,
                                            filename, MediaFile.ManagementType.TO_BE_CLONED, source_loc))

    def find_all_deck_part_media_files(self):
        self.known_media_files_dict = {}

        for full_path in find_all_files_in_directory(self.global_config.deck_parts.media_files, recursive=True):
            filename = filename_from_full_path(full_path)
            self._register_media_file(MediaFile(full_path, filename))

        logging.debug(f"Media files found: {len(self.known_media_files_dict)}")

    def write_to_all(self):
        files_to_create = []
        for location, file in self.known_files_dict.items():
            if not file.file_exists:
                files_to_create.append(location)

        # logging.info(f"Will create {len(files_to_create)} new files: ", files_to_create)

        for write_file in self.write_files_at_end:
            write_file.write_file_on_close()

        for location, file in self.known_files_dict.items():
            if file.data_state == GenericFile.DataState.DATA_SET:
                logging.info(f"Wrote to {file.file_location}")
                file.write_file()
                file.data_state = GenericFile.DataState.READ_IN_DATA

        for filename, media_file in self.known_media_files_dict.items():
            media_file.copy_source_to_target()
