from typing import Dict

from brain_brew.representation.generic.generic_file import GenericFile


class FileManager:
    __instance = None

    known_files_dict: Dict[str, GenericFile] = {}

    @classmethod
    def get_instance(cls, override=None):
        if override:
            cls.__instance = override
        return cls.__instance or cls()

    def file_if_exists(self, file_location):
        if file_location in self.known_files_dict.keys():
            return self.known_files_dict[file_location]
        return None

    def register_file(self, file_location, file):
        if file_location in self.known_files_dict:
            raise FileExistsError("File already known to FileManager, cannot be registered twice")
        self.known_files_dict.setdefault(file_location, file)

    def write_to_all(self):
        files_to_create = []
        for location, file in self.known_files_dict.items():
            if not file.file_exists:
                files_to_create.append(location)

        print(f"Will create {len(files_to_create)} new files: ", files_to_create)

        for location, file in self.known_files_dict.items():
            if file.data_state == GenericFile.DataState.DATA_SET:
                print(f"Wrote to {file.file_location}")
                file.write_file()
                file.data_state = GenericFile.DataState.READ_IN_DATA

