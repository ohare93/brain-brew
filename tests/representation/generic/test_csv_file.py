import pytest

from brain_brew.representation.generic.csv_file import CsvFile
from tests.test_file_manager import get_new_file_manager
from tests.test_files import TestFiles

get_new_file_manager()


@pytest.fixture()
def csv_test1():
    csv = CsvFile(TestFiles.CsvFiles.TEST1)
    csv.read_file()
    return csv


@pytest.fixture()
def tsv_test1():
    tsv = CsvFile(TestFiles.TsvFiles.TEST1, delimiter='\t')
    tsv.read_file()
    return tsv


@pytest.fixture()
def csv_test1_split1():
    csv = CsvFile(TestFiles.CsvFiles.TEST1_SPLIT1)
    csv.read_file()
    return csv


@pytest.fixture()
def csv_test1_split2():
    csv = CsvFile(TestFiles.CsvFiles.TEST1_SPLIT2)
    csv.read_file()
    return csv


@pytest.fixture()
def csv_test2():
    csv = CsvFile(TestFiles.CsvFiles.TEST2)
    csv.read_file()
    return csv


@pytest.fixture()
def csv_test3():
    csv = CsvFile(TestFiles.CsvFiles.TEST3)
    csv.read_file()
    return csv


@pytest.fixture()
def csv_test2_missing_guids():
    csv = CsvFile(TestFiles.CsvFiles.TEST2_MISSING_GUIDS)
    csv.read_file()
    return csv


@pytest.fixture()
def temp_csv_test1(tmpdir, csv_test1) -> CsvFile:
    file = tmpdir.mkdir("json").join("file.csv")
    file.write("blank")

    csv = CsvFile.create_or_get(file.strpath)
    csv.read_file()
    return csv


class TestConstructor:
    def test_runs(self, csv_test1):
        assert isinstance(csv_test1, CsvFile)
        assert csv_test1.file_location == TestFiles.CsvFiles.TEST1
        assert "guid" in csv_test1.column_headers


def test_to_filename_csv():
    assert "read-this-file.csv" == CsvFile.to_filename_csv("read-this-file")
    assert "read-this-file.csv" == CsvFile.to_filename_csv("read-this-file.csv")
    assert "read-this-file.tsv" == CsvFile.to_filename_csv("read-this-file.tsv")


class TestWriteFile:
    def test_runs(self, temp_csv_test1: CsvFile, csv_test1: CsvFile):
        temp_csv_test1.set_data(csv_test1.get_data())
        temp_csv_test1.write_file()
        temp_csv_test1.read_file()

        assert temp_csv_test1.get_data() == csv_test1.get_data()

    def test_tsv_same_data(self, temp_csv_test1: CsvFile, tsv_test1: CsvFile):
        temp_csv_test1.set_data(tsv_test1.get_data())
        temp_csv_test1.write_file()
        temp_csv_test1.read_file()

        assert temp_csv_test1.get_data() == tsv_test1.get_data()


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
        csv_test1.sort_data(columns, reverse, case_insensitive_sort=True)

        sorted_data = csv_test1.get_data()

        for result in expected_results:
            assert sorted_data[result[0]][result_column] == result[1]

    def test_insensitive(self):
        pass
