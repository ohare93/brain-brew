import abc


class WritesFile(abc.ABC):
    @abc.abstractmethod
    def write_file_on_close(self):
        pass

    def __init__(self):
        from brain_brew.file_manager import FileManager
        fm = FileManager.get_instance()
        fm.register_write_file_for_end(self)
