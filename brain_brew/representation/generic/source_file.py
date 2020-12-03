import copy
from pathlib import Path


class SourceFile(object):
    @classmethod
    def from_file_loc(cls, file_loc) -> 'SourceFile':
        pass

    @classmethod
    def is_file(cls, filename: str):
        return Path(filename).is_file()

    @classmethod
    def is_dir(cls, folder_name: str):
        return Path(folder_name).is_dir()

    @classmethod
    def get_deep_copy(cls, data):
        return copy.deepcopy(data)

    @classmethod
    def create_or_get(cls, location):
        from brain_brew.configuration.file_manager import FileManager
        _file_manager = FileManager.get_instance()
        formatted_location = cls.formatted_file_location(location)
        file = _file_manager.file_if_exists(formatted_location)

        if file is not None:
            return file

        # if not cls.is_file(location) and not cls.is_dir(location):
        #     raise FileNotFoundError(f"No file or folder '{location}' exists")

        file = cls.from_file_loc(location)
        _file_manager.register_file(formatted_location, file)
        return file

    @classmethod
    def formatted_file_location(cls, location):
        return location
