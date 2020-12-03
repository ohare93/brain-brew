import pytest

from brain_brew.representation.generic.media_file import MediaFile


@pytest.fixture()
def media_file_test1() -> MediaFile:
    return MediaFile("place/loc/file.txt")


class TestConstructor:
    def test_without_override(self):
        loc = "place/loc/file.txt"

        media_file = MediaFile(loc)

        assert isinstance(media_file, MediaFile)
        assert media_file.file_path == loc
        assert media_file.filename == "file.txt"


class TestCopy:
    def test_copies_file(self, tmpdir):
        source_dir = tmpdir.mkdir("source")
        source = source_dir.join("test.txt")
        source.write("test content")
        assert len(source_dir.listdir()) == 1

        target_dir = tmpdir.mkdir("target")
        target = target_dir.join("test.txt")
        assert len(target_dir.listdir()) == 0

        media_file = MediaFile(str(source))
        media_file.copy_self_to_target(str(target))

        assert len(target_dir.listdir()) == len(source_dir.listdir()) == 1
        assert target.read() == "test content"
