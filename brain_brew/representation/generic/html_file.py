from dataclasses import dataclass

from brain_brew.representation.generic.source_file import SourceFile

_encoding = "utf-8"

@dataclass
class HTMLFile(SourceFile):
    file_location: str
    _data: str

    def __init__(self, file):
        self.file_location = file
        self.read_file()

    @classmethod
    def from_file_loc(cls, file_loc) -> 'HTMLFile':
        return cls(file_loc)

    def read_file(self):
        r = open(self.file_location, 'r', encoding=_encoding)
        self._data = r.read()

    def get_data(self, deep_copy=False) -> str:
        return self.get_deep_copy(self._data) if deep_copy else self._data

    @staticmethod
    def write_file(file_location, data):
        with open(file_location, "w+", encoding=_encoding) as file:
            file.write(data)

    @staticmethod
    def to_filename_html(filename: str) -> str:
        return filename + ".html" if not filename.endswith(".html") else filename

    @classmethod
    def formatted_file_location(cls, location):
        return cls.to_filename_html(location)
