from unittest.mock import patch

import pytest

from brain_brew.representation.generic.csv_file import CsvFile, CsvKeys
from brain_brew.representation.generic.source_file import SourceFile
from tests.test_files import TestFiles


@pytest.fixture()
def csv_test1():
    return CsvFile(TestFiles.CsvFiles.TEST1)


@pytest.fixture()
def csv_test1_split1():
    return CsvFile(TestFiles.CsvFiles.TEST1_SPLIT1)


@pytest.fixture()
def csv_test1_split2():
    return CsvFile(TestFiles.CsvFiles.TEST1_SPLIT2)


@pytest.fixture()
def csv_test2():
    return CsvFile(TestFiles.CsvFiles.TEST2)


@pytest.fixture()
def csv_test3():
    return CsvFile(TestFiles.CsvFiles.TEST3)


@pytest.fixture()
def csv_test1_not_read_initially_test():
    return CsvFile(TestFiles.CsvFiles.TEST1, read_now=False)


@pytest.fixture()
def csv_test2_missing_guids():
    return CsvFile(TestFiles.CsvFiles.TEST2_MISSING_GUIDS)


@pytest.fixture()
def temp_csv_test1(tmpdir, csv_test1) -> CsvFile:
    file = tmpdir.mkdir("json").join("file.csv")
    file.write("blank")

    return CsvFile.create_or_get(file.strpath, data_override=csv_test1.get_data())


class TestConstructor:
    def test_runs(self, csv_test1):
        assert isinstance(csv_test1, CsvFile)
        assert csv_test1.file_location == TestFiles.CsvFiles.TEST1
        assert "guid" in csv_test1.column_headers

    @pytest.mark.parametrize("column_headers", [
        ["first", "second", "third", "etc"],
        ["Word", "OtherWord", "Audio", "Test"],
        ["X", "Field name with spaces", "", "MorphMan_FocusMorph"],
    ])
    def test_data_override(self, column_headers):
        data_override = [{key: "value" for key in column_headers}]
        csv = CsvFile("file", data_override=data_override)

        assert csv.column_headers == column_headers


def test_to_filename_csv():
    expected = "read-this-file.csv"

    assert expected == CsvFile.to_filename_csv("read this file")
    assert expected == CsvFile.to_filename_csv("read-this-file")
    assert expected == CsvFile.to_filename_csv("read-this-file.csv")
    assert expected == CsvFile.to_filename_csv("read          this        file")


class TestReadFile:
    def test_runs(self, csv_test1_not_read_initially_test):
        assert csv_test1_not_read_initially_test.get_data() == []
        assert csv_test1_not_read_initially_test.column_headers == []
        assert csv_test1_not_read_initially_test.file_location == TestFiles.CsvFiles.TEST1
        assert csv_test1_not_read_initially_test.data_state == SourceFile.DataState.NOTHING_READ_OR_SET

        csv_test1_not_read_initially_test.read_file()

        assert len(csv_test1_not_read_initially_test.get_data()) == 15
        assert "guid" in csv_test1_not_read_initially_test.column_headers
        assert csv_test1_not_read_initially_test.data_state == SourceFile.DataState.READ_IN_DATA


class TestWriteFile:
    def test_runs(self, temp_csv_test1: CsvFile, csv_test1: CsvFile):
        temp_csv_test1.write_file()
        temp_csv_test1.read_file()

        assert temp_csv_test1.get_data() == csv_test1.get_data()


class TestSortData:
    @pytest.mark.parametrize("columns, reverse, result_column, expected_results", [
        (["guid"], False, "guid", [(0, "AAAA"), (1, "BBBB"), (2, "CCCC"), (14, "OOOO")]),
        (["guid"], True, "guid", [(14, "AAAA"), (13, "BBBB"), (12, "CCCC"), (0, "OOOO")]),
        (["english"], False, "english", [(0, "banana"), (1, "bird"), (2, "cat"), (14, "you")]),
        (["english"], True, "english", [(14, "banana"), (13, "bird"), (12, "cat"), (0, "you")]),
        (["tags"], False, "tags", [(0, "besttag"), (1, "funny"), (2, "tag2 tag3"), (13, ""), (14, "")]),
        (["esperanto", "english"], False, "esperanto", [(0, "banano"), (1, "birdo"), (6, "vi"), (14, "")]),
        (["esperanto", "guid"], False, "guid", [(7, "BBBB"), (14, "LLLL")]),
    ])
    def test_sort(self, csv_test1: CsvFile, columns, reverse, result_column, expected_results):
        csv_test1.sort_data(columns, reverse)

        sorted_data = csv_test1.get_data()

        for result in expected_results:
            assert sorted_data[result[0]][result_column] == result[1]

    def test_insensitive(self):
        pass
