from brain_brew.representation.generic.csv_file import CsvFile
from tests.test_files import TestFiles


class TestConstructor:
    def test_runs(self):
        file_location = TestFiles.CsvFiles.TEST1
        csv = CsvFile(file_location)

        assert isinstance(csv, CsvFile)
        assert csv.file_location == file_location
        assert "guid" in csv.column_headers
        assert len(csv._data) == 15


class TestGetRelevantData:
    def test_data_correct(self):
        file_location = TestFiles.CsvFiles.TEST1
        csv = CsvFile(file_location)

        relevant_columns = ["guid", "english", "danish"]
        data = csv.get_relevant_data(relevant_columns)

        assert isinstance(data, list)
        assert file_location == csv.file_location
        assert len(data) == 15
        for row in data:
            assert len(row) == 3
            for column in relevant_columns:
                assert column in row

    def test_capitilisation_ignored(self):
        file_location = TestFiles.CsvFiles.TEST1
        csv = CsvFile(file_location)

        relevant_columns = ["GUID", "English", "DaNiSh"]
        data = csv.get_relevant_data(relevant_columns)

        assert isinstance(data, list)
        assert list(data[0].keys()) == ["guid", "english", "danish"]

    def test_nothing_relevant_no_data(self):
        file_location = TestFiles.CsvFiles.TEST1
        csv = CsvFile(file_location)

        relevant_columns = []
        data = csv.get_relevant_data(relevant_columns)

        assert isinstance(data, list)
        assert len(data) == 0
