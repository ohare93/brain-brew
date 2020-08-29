import copy
from pathlib import Path

from brain_brew.representation.configuration.global_config import GlobalConfig


class SourceFile:
    @classmethod
    def from_file_loc(cls, file_loc) -> 'SourceFile':
        pass

    @classmethod
    def is_file(cls, filename: str):
        return Path(filename).is_file()

    @classmethod
    def is_dir(cls, folder_name: str):
        return Path(folder_name).is_file()

    @classmethod
    def get_deep_copy(cls, data):
        return copy.deepcopy(data)

    @classmethod
    def create_or_get(cls, location):
        from brain_brew.file_manager import FileManager
        _file_manager = FileManager.get_instance()
        formatted_location = cls.formatted_file_location(location)
        file = _file_manager.file_if_exists(formatted_location)

        if file is not None:
            return file

        file = cls.from_file_loc(location)
        _file_manager.register_file(formatted_location, file)
        return file

    @classmethod
    def formatted_file_location(cls, location):
        return location

    @staticmethod
    def _sort_data(data, sort_by_keys, reverse_sort, case_insensitive_sort=None):  # TODO: Move to NoteGroupings
        if case_insensitive_sort is None:
            case_insensitive_sort = GlobalConfig.get_instance().defaults.sort_case_insensitive

        if sort_by_keys:
            if case_insensitive_sort:
                def sort_method(i): return tuple((i[column] == "", i[column].lower()) for column in sort_by_keys)
            else:
                def sort_method(i): return tuple((i[column] == "", i[column]) for column in sort_by_keys)

            return sorted(data, key=sort_method, reverse=reverse_sort)
        elif reverse_sort:
            return list(reversed(data))

        return data
