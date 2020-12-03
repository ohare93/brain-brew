from brain_brew.configuration.file_manager import FileManager


def get_new_file_manager():
    FileManager.clear_instance()
    return FileManager()


# class TestSingletonConstructor:
#     def test_runs(self, global_config):
#         fm = get_new_file_manager()
#         assert isinstance(fm, FileManager)
#
#     def test_returns_existing_singleton(self):
#         fm = get_new_file_manager()
#         fm.known_files_dict = {'test': None}
#         fm2 = FileManager.get_instance()
#
#         assert fm2.known_files_dict == {'test': None}
#         assert fm2 == fm
#
#     def test_raises_error(self):
#         with pytest.raises(Exception):
#             FileManager()
#             FileManager()
#
#
# class TestFindMediaFiles:
#     def test_finds(self):
#         fm = get_new_file_manager()
#
#         assert len(fm.known_media_files_dict) == 2
