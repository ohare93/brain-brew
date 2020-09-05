import logging

import pytest
from unittest.mock import MagicMock

from brain_brew.representation.generic.source_file import SourceFile
from tests.test_file_manager import get_new_file_manager
from tests.test_files import TestFiles


class TestConstructor:
    def test_runs(self):
        file_location = TestFiles.CsvFiles.TEST1
        file = SourceFile(file_location, read_now=False, data_override=None)

        assert isinstance(file, SourceFile)
        assert file.file_location == file_location
        assert file._data is None

    def test_no_file_found(self):
        file_location = "sdfsdfgdsfsdfsdsdg/sdfsdf/sdfsdf/sdfsd/"

        with pytest.raises(FileNotFoundError):
            SourceFile(file_location, read_now=True, data_override=None)

    def test_override_data(self):
        override_data = {"Test": 1}
        file = SourceFile("", read_now=True, data_override=override_data)

        assert isinstance(file, SourceFile)
        assert file._data == override_data


class TestCreateFileWithFileManager:
    def test_runs(self):
        fm = get_new_file_manager()
        assert len(fm.known_files_dict) == 0

        first = SourceFile.create_or_get("test1", read_now=False)

        assert isinstance(first, SourceFile)
        assert len(fm.known_files_dict) == 1
        assert fm.known_files_dict["test1"]

    def test_returns_existing_object(self):
        fm = get_new_file_manager()
        assert len(fm.known_files_dict) == 0

        first = SourceFile.create_or_get("test1", read_now=False)
        second = SourceFile.create_or_get("test1", read_now=False)

        assert first == second
        assert len(fm.known_files_dict) == 1
