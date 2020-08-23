import glob
import logging
from typing import List, Dict

from brain_brew.representation.generic.generic_file import SourceFile
from brain_brew.representation.generic.media_file import MediaFile
from brain_brew.representation.json.json_file import JsonFile
from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiJsonWrapper
from brain_brew.utils import filename_from_full_path, find_all_files_in_directory


class CrowdAnkiExport(SourceFile):
    folder_location: str
    json_file_location: str
    # import_config: CrowdAnkiImportConfig  # TODO: Make this

    contains_media: bool
    known_media: Dict[str, MediaFile]
    media_loc: str

    def __init__(self, folder_location):
        self.folder_location = folder_location
        if self.folder_location[-1] != "/":
            self.folder_location = self.folder_location + "/"

        if not self.is_dir(self.folder_location):
            raise FileNotFoundError(f"Missing CrowdAnkiExport '{self.folder_location}'")

        self.json_file_location = self.find_json_file_in_folder()
        self.find_all_media()

    @classmethod
    def from_file_loc(cls, file_loc) -> 'CrowdAnkiExport':
        return cls(file_loc)

    def find_json_file_in_folder(self):
        files = glob.glob(self.folder_location + "*.json")

        if len(files) == 1:
            return files[0]
        elif not files:
            logging.error(f"No json file found in folder '{self.folder_location}'")
            raise FileNotFoundError(self.folder_location)
        else:
            logging.error(f"Multiple json files found in '{self.folder_location}': {files}")
            raise FileExistsError()

    def find_all_media(self):
        self.known_media = {}
        self.media_loc = self.folder_location + "media/"  # TODO: Make media folder if not exists
        self.contains_media = self.is_dir(self.media_loc)

        if not self.contains_media:
            return

        media_files = find_all_files_in_directory(self.media_loc)

        for full_path in media_files:
            filename = filename_from_full_path(full_path)
            self.known_media.setdefault(filename, MediaFile(full_path, filename))

        logging.info(f"CrowdAnkiExport found {len(self.known_media)} media files in folder")

    def write_to_files(self, json_data):  # import_config_data
        JsonFile.write_file(self.json_file_location, json_data)
        for filename, media_file in self.known_media.items():
            media_file.copy_source_to_target()

    def read_json_file(self) -> CrowdAnkiJsonWrapper:
        return CrowdAnkiJsonWrapper(JsonFile.read_file(self.json_file_location))
