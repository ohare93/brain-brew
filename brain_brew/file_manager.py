from typing import Dict, Union

from brain_brew.representation.generic.source_file import SourceFile
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.yaml_object import YamlObject


class FileManager:
    __instance = None

    known_files_dict: Dict[str, SourceFile]
    deck_part_pool: Dict[str, DeckPartHolder[YamlObject]]

    def __init__(self):
        if FileManager.__instance is None:
            FileManager.__instance = self
        else:
            raise Exception("Multiple FileManagers created")

        self.known_files_dict = {}
        self.deck_part_pool = {}

    @staticmethod
    def get_instance() -> 'FileManager':
        return FileManager.__instance

    @staticmethod
    def clear_instance():
        if FileManager.__instance:
            FileManager.__instance = None

    # If Exists
    def file_if_exists(self, file_location) -> Union[SourceFile, None]:
        if file_location in self.known_files_dict.keys():
            return self.known_files_dict[file_location]
        return None

    def deck_part_if_exists(self, dp_name) -> Union[DeckPartHolder[YamlObject], None]:
        return self.deck_part_pool.get(dp_name)

    # Registration
    def register_file(self, full_path, file):
        if full_path in self.known_files_dict:
            raise FileExistsError(f"File already known to FileManager, cannot be registered twice: {full_path}")
        self.known_files_dict.setdefault(full_path, file)

    def new_deck_part(self, dp: DeckPartHolder) -> DeckPartHolder:
        if dp.part_id in self.deck_part_pool:
            raise KeyError(f"Cannot use same name '{dp.part_id}' for multiple Deck Parts")
        self.deck_part_pool.setdefault(dp.part_id, dp)
        return dp

    def deck_part_from_pool(self, name: str):
        if name not in self.deck_part_pool:
            raise KeyError(f"Cannot find Deck Part '{name}'")
        return self.deck_part_pool[name]
