from brain_brew.representation.json.json_file import JsonFile
from tests.test_files import TestFiles


def test_constructor():
    file_location = TestFiles.CrowdAnkiExport.TEST1_JSON
    file = JsonFile(file_location)

    assert isinstance(file, JsonFile)
    assert file.file_location == file_location
    assert len(file.get_data().keys()) == 7


def test_to_filename_json():
    expected = "read-this-file.json"

    assert expected == JsonFile.to_filename_json("read this file")
    assert expected == JsonFile.to_filename_json("read-this-file")
    assert expected == JsonFile.to_filename_json("read-this-file.json")
    assert expected == JsonFile.to_filename_json("read          this        file")


def test_configure_file_location():
    expected = "folder/read-this-file.json"

    assert expected == JsonFile.get_json_file_location("folder/", "read this file")
    assert expected == JsonFile.get_json_file_location("folder/", "read-this-file.json")
    assert expected == JsonFile.get_json_file_location("", "folder/read this file")
    assert expected == JsonFile.get_json_file_location("", "folder/read-this-file.json")
