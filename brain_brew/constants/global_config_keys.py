from enum import Enum


class ConfigKeys(Enum):
    AUTHORS = "authors"

    DECK_PARTS = "deck_parts"
    HEADERS = "headers"
    NOTE_MODELS = "note_models"
    NOTES = "notes"
    MEDIA_FILES = "media_files"

    FLAGS = "flags"
    SORT_CASE_INSENSITIVE = "sort_case_insensitive"
    JOIN_VALUES_WITH = "join_values_with"

    DECK_PARTS_NOTES_STRUCTURE = "deck_part_notes_structure"
    NOTE_SORT_ORDER = "note_sort_order"
    REVERSE_SORT = "reverse_sort"
