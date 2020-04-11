import pytest

from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from tests.test_files import TestFiles


class TestConstructor:
    @pytest.mark.parametrize("export_name", [
        TestFiles.CrowdAnkiExport.TEST1_FOLDER,
        TestFiles.CrowdAnkiExport.TEST1_FOLDER_WITHOUT_SLASH
    ])
    def test_runs(self, export_name):
        file = CrowdAnkiExport(export_name)

        assert isinstance(file, CrowdAnkiExport)
        assert file.folder_location == TestFiles.CrowdAnkiExport.TEST1_FOLDER
        assert file.file_location == TestFiles.CrowdAnkiExport.TEST1_JSON
        assert len(file.get_data().keys()) == 7


class TestFindJsonFileInFolder:
    def test_no_json_file(self, tmpdir):
        directory = tmpdir.mkdir("test")

        with pytest.raises(FileNotFoundError):
            file = CrowdAnkiExport(directory.strpath)

    def test_too_many_json_files(self, tmpdir):
        directory = tmpdir.mkdir("test")
        file1, file2 = directory.join("file1.json"), directory.join("file2.json")
        file1.write("{}")
        file2.write("{}")

        with pytest.raises(FileExistsError):
            file = CrowdAnkiExport(directory.strpath)


@pytest.fixture()
def ca_export_test1():
    return CrowdAnkiExport.create(TestFiles.CrowdAnkiExport.TEST1_FOLDER)
