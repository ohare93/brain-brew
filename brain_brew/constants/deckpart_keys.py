from dataclasses import dataclass
from enum import Enum


class DeckPartNoteKeys(Enum):
    NOTES = "notes"
    FIELDS = "fields"
    GUID = "guid"
    NOTE_MODEL = "note_model"
    # NOTE_MODEL_GROUPED = "_sharednotemodel"
    TAGS = "tags"
    SHARED_TAGS = "_shared_tags"
    FLAGS = "_flags"


class NoteFlagKeys(Enum):
    GROUP_BY_NOTE_MODEL = "group_by_note_model"
    EXTRACT_SHARED_TAGS = "extract_shared_tags"


@dataclass
class DeckPartNoteFlags:
    group_by_note_model: bool = False
    extract_shared_tags: bool = False

    @staticmethod
    def as_formatted_dict(group_by_note_model: bool, extract_shared_tags: bool):
        return {DeckPartNoteKeys.FLAGS.value: {
            NoteFlagKeys.EXTRACT_SHARED_TAGS.value: extract_shared_tags,
            NoteFlagKeys.GROUP_BY_NOTE_MODEL.value: group_by_note_model
        }}

    def get_formatted_dict(self):
        return DeckPartNoteFlags.as_formatted_dict(self.group_by_note_model, self.extract_shared_tags)

    def any_enabled(self):
        return self.group_by_note_model or self.extract_shared_tags
