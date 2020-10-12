import glob
import logging
from typing import List, Dict

from brain_brew.representation.generic.source_file import SourceFile
from brain_brew.representation.generic.media_file import MediaFile
from brain_brew.representation.json.json_file import JsonFile
from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiJsonWrapper
from brain_brew.representation.yaml.note_model_repr import NoteModel
from brain_brew.utils import filename_from_full_path, find_all_files_in_directory, create_path_if_not_exists


class CrowdAnkiExport(SourceFile):
    folder_location: str
    json_file_location: str
    # import_config: CrowdAnkiImportConfig  # TODO: Make this
    json_data: CrowdAnkiJsonWrapper
    note_models: List[NoteModel]

    contains_media: bool
    known_media: Dict[str, MediaFile]
    media_loc: str

    def __init__(self, folder_location):
        self.folder_location = folder_location
        if self.folder_location[-1] != "/":
            self.folder_location = self.folder_location + "/"

        create_path_if_not_exists(self.folder_location)

        self.json_file_location = self.find_json_file_in_folder()
        self._read_json_file()
        self.find_all_media()

    @classmethod
    def from_file_loc(cls, file_loc) -> 'CrowdAnkiExport':
        return cls(file_loc)

    def find_json_file_in_folder(self):
        files = glob.glob(self.folder_location + "*.json")

        if len(files) == 1:
            return files[0]
        elif not files:
            file_loc = self.folder_location + self.folder_location.split("/")[-2] + ".json"
            logging.warning(f"Creating missing json file '{file_loc}'")
            return file_loc
        else:
            logging.error(f"Multiple json files found in '{self.folder_location}': {files}")
            raise FileExistsError()

    def find_all_media(self):
        self.known_media = {}
        self.media_loc = self.folder_location + "media/"
        self.contains_media = self.is_dir(self.media_loc)

        if not self.contains_media:
            create_path_if_not_exists(self.media_loc)
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

    def _read_json_file(self):
        self.json_data = CrowdAnkiJsonWrapper(JsonFile.read_file(self.json_file_location))
        self.note_models = list(map(NoteModel.from_crowdanki, self.json_data.note_models))
