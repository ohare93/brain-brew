from typing import Dict, Union

from brain_brew.configuration.part_holder import PartHolder
from brain_brew.representation.generic.source_file import SourceFile
from brain_brew.representation.yaml.yaml_object import YamlObject


class FileManager:
    __instance = None

    known_files_dict: Dict[str, SourceFile]
    known_parts: Dict[str, PartHolder[YamlObject]]

    def __init__(self):
        if FileManager.__instance is None:
            FileManager.__instance = self
        else:
            raise Exception("Multiple FileManagers created")

        self.known_files_dict = {}
        self.known_parts = {}

    @staticmethod
    def get_instance() -> 'FileManager':
        return FileManager.__instance

    @staticmethod
    def clear_instance():
        if FileManager.__instance:
            FileManager.__instance = None

    # Source Files

    def register_file(self, full_path, file):
        if full_path in self.known_files_dict:
            raise FileExistsError(f"File already known to FileManager, cannot be registered twice: {full_path}")
        self.known_files_dict.setdefault(full_path, file)

    def file_if_exists(self, file_location) -> Union[SourceFile, None]:
        if file_location in self.known_files_dict.keys():
            return self.known_files_dict[file_location]
        return None

    # Deck Parts

    def register_part(self, dp: PartHolder) -> PartHolder:
        if dp.part_id in self.known_parts:
            raise KeyError(f"Cannot use same name '{dp.part_id}' for multiple Deck Parts")
        self.known_parts.setdefault(dp.part_id, dp)
        return dp

    def get_part_if_exists(self, dp_name) -> Union[PartHolder[YamlObject], None]:
        return self.known_parts.get(dp_name)

    def get_part(self, name: str):
        if name not in self.known_parts:
            raise KeyError(f"Cannot find Deck Part '{name}'")
        return self.known_parts[name]
