import logging
from dataclasses import dataclass
from typing import Optional, TypeVar, Generic

T = TypeVar('T')


@dataclass
class DeckPartHolder(Generic[T]):
    part_id: str
    save_to_file: Optional[str]
    deck_part: T

    file_manager = None

    @classmethod
    def get_file_manager(cls):
        if not cls.file_manager:
            from brain_brew.file_manager import FileManager
            cls.file_manager = FileManager.get_instance()
        return cls.file_manager

    @classmethod
    def from_file_manager(cls, part_id: str) -> T:
        return cls.get_file_manager().get_deck_part(part_id)

    @classmethod
    def override_or_create(cls, part_id: str, save_to_file: Optional[str], deck_part: T):
        fm = cls.get_file_manager()

        dp = fm.get_deck_part_if_exists(part_id)
        if dp is None:
            dp = fm.register_deck_part(DeckPartHolder(part_id, save_to_file, deck_part))
        else:
            logging.warning(f"Overwriting existing Deck Part '{part_id}'")
            dp.deck_part = deck_part
            dp.save_to_file = save_to_file

        dp.write_to_file()

        return dp

    def write_to_file(self):
        if self.save_to_file is not None:
            self.deck_part.dump_to_yaml(self.save_to_file)
