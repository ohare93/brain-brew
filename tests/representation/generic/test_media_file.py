import shutil
from unittest.mock import patch

import pytest

from brain_brew.representation.generic.media_file import MediaFile


@pytest.fixture()
def media_file_test1() -> MediaFile:
    return MediaFile("loc", "name")


class TestConstructor:
    def test_without_override(self):
        loc = "test loc"
        name = "name"

        media_file = MediaFile(loc, name)

        assert isinstance(media_file, MediaFile)
        assert media_file.source_loc == media_file.file_location == loc
        assert media_file.filename == name
        assert media_file.man_type == MediaFile.ManagementType.EXISTS

    def test_with_override(self):
        source_loc = "test loc"
        target_loc = "other loc"
        name = "name"
        man_type = MediaFile.ManagementType.TO_BE_CLONED

        media_file = MediaFile(target_loc, name, man_type, source_loc)

        assert isinstance(media_file, MediaFile)
        assert media_file.file_location == target_loc
        assert media_file.source_loc == source_loc
        assert media_file.filename == name
        assert media_file.man_type == man_type


def test_set_override(media_file_test1):
    assert media_file_test1.man_type == MediaFile.ManagementType.EXISTS
    assert media_file_test1.source_loc == media_file_test1.file_location == "loc"

    media_file_test1.set_override("new loc")

    assert media_file_test1.source_loc == "new loc"
    assert media_file_test1.file_location == "loc"
    assert media_file_test1.man_type == MediaFile.ManagementType.OVERRIDDEN


@pytest.mark.parametrize("man_type, expected_result", [
    (MediaFile.ManagementType.EXISTS, False),
    (MediaFile.ManagementType.OVERRIDDEN, True),
    (MediaFile.ManagementType.TO_BE_CLONED, True),
])
def test_should_write(media_file_test1, man_type, expected_result):
    media_file_test1.man_type = man_type
    assert media_file_test1.should_write() == expected_result


class TestCopy:
    @pytest.mark.parametrize("should_write_returns, num_calls_to_copy", [
        (True, 1),
        (False, 0)
    ])
    def test_takes_should_write_into_account(self, media_file_test1, should_write_returns, num_calls_to_copy):
        with patch.object(MediaFile, "should_write", return_value=should_write_returns), \
                patch.object(shutil, "copy2") as mock_copy:
            media_file_test1.copy_source_to_target()
            assert mock_copy.call_count == num_calls_to_copy

    def test_copies_file(self, tmpdir):
        source_dir = tmpdir.mkdir("source")
        source = source_dir.join("test.txt")
        source.write("test content")
        assert len(source_dir.listdir()) == 1

        target_dir = tmpdir.mkdir("target")
        target = target_dir.join("test.txt")
        assert len(target_dir.listdir()) == 0

        media_file = MediaFile(target, "test.txt", MediaFile.ManagementType.TO_BE_CLONED, source)
        media_file.copy_source_to_target()

        assert len(target_dir.listdir()) == len(source_dir.listdir()) == 1
        assert target.read() == "test content"
