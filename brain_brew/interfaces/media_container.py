from abc import ABC, abstractmethod
from typing import Set


class MediaContainer(ABC):
    @abstractmethod
    def get_all_media_references(self) -> Set[str]:
        pass
