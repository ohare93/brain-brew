import logging

import pytest
from unittest.mock import MagicMock

from brain_brew.representation.generic.generic_file import GenericFile
from tests.test_files import TestFiles


class TestConstructor:
    def test_runs(self):
        file_location = TestFiles.CsvFiles.TEST1
        file = GenericFile(file_location, read_now=False, data_override=None)

        assert isinstance(file, GenericFile)
        assert file.file_location == file_location
        assert file._data is None

    def test_no_file_found(self):
        file_location = "sdfsdfgdsfsdfsdsdg/sdfsdf/sdfsdf/sdfsd/"

        with pytest.raises(FileNotFoundError):
            GenericFile(file_location, read_now=True, data_override=None)

    def test_override_data(self):
        override_data = {"Test": 1}
        file = GenericFile("", read_now=True, data_override=override_data)

        assert isinstance(file, GenericFile)
        assert file._data == override_data
