import glob
import logging

from brain_brew.representation.json.json_file import JsonFile


class CrowdAnkiExport(JsonFile):
    folder_location: str

    def __init__(self, folder_location, read_now=True, data_override=None):
        self.folder_location = folder_location
        if self.folder_location[-1] != "/":
            self.folder_location = self.folder_location + "/"

        json_file_location = self.find_json_file_in_folder()

        super().__init__(json_file_location, read_now=read_now, data_override=data_override)

    def find_json_file_in_folder(self):
        files = glob.glob(self.folder_location + "*.json")
        # print("Files", files)

        if len(files) == 1:
            return files[0]
        elif not files:
            logging.error(f"No json file found in folder '{self.folder_location}'")
            raise FileNotFoundError(self.folder_location)
        else:
            print(f"Multiple json files found in '{self.folder_location}': {files}")
            raise FileExistsError()

