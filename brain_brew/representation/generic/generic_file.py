import copy
from enum import Enum
from pathlib import Path

from brain_brew.representation.configuration.global_config import GlobalConfig


class GenericFile:
    class DataState(Enum):
        NOTHING_READ_OR_SET = 0
        READ_IN_DATA = 1
        DATA_SET = 2

    _data = None
    file_location: str

    file_exists: bool
    data_state: DataState = DataState.NOTHING_READ_OR_SET

    def __init__(self, file, read_now, data_override):
        self.file_location = file

        self.file_exists = Path(file).is_file()

        if data_override:
            self.data_state = GenericFile.DataState.DATA_SET
            self.set_data(data_override)
        elif read_now:
            if not self.file_exists:
                return  # raise FileNotFoundError(file)  # TODO: Fix
            self.data_state = GenericFile.DataState.READ_IN_DATA
            self.read_file()

    def set_data(self, data_override):
        self.data_state = GenericFile.DataState.DATA_SET
        self._data = data_override

    def get_data(self, deep_copy: bool = False):
        return copy.deepcopy(self._data) if deep_copy else self._data

    def read_file(self):
        raise NotImplemented

    def write_file(self):
        raise NotImplemented

    @classmethod
    def create(cls, location, read_now=True, data_override=None):
        from brain_brew.file_manager import FileManager
        _file_manager = FileManager.get_instance()
        formatted_location = cls.formatted_file_location(location)
        file = _file_manager.file_if_exists(formatted_location)

        if file is not None:
            return file

        file = cls(location, read_now=read_now, data_override=data_override)
        _file_manager.register_file(formatted_location, file)
        return file

    @classmethod
    def formatted_file_location(cls, location):
        return location

    @staticmethod
    def _sort_data(data, sort_by_keys, reverse_sort, case_insensitive_sort=None):
        if case_insensitive_sort is None:
            case_insensitive_sort = GlobalConfig.get_instance().flags.sort_case_insensitive

        if sort_by_keys:
            if case_insensitive_sort:
                def sort_method(i): return tuple((i[column] == "", i[column].lower()) for column in sort_by_keys)
            else:
                def sort_method(i): return tuple((i[column] == "", i[column]) for column in sort_by_keys)

            return sorted(data, key=sort_method, reverse=reverse_sort)
        elif reverse_sort:
            return list(reversed(data))

        return data
