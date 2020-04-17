from enum import Enum
from typing import List

from brain_brew.constants.deckpart_keys import DeckPartNoteKeys
from brain_brew.representation.generic.yaml_file import YamlFile, ConfigKey
from brain_brew.representation.json.deck_part_notemodel import DeckPartNoteModel


class NoteModelMappingKeys(Enum):
    NOTE_MODEL = "note_model"
    COLUMNS = "csv_columns_to_fields"
    PERSONAL_FIELDS = "personal_fields"


class FieldMapping:
    class FieldMappingType(Enum):
        COLUMN = "column"
        PERSONAL_FIELD = "personal_field"
        DEFAULT = "default"

        @classmethod
        def values(cls):
            return set(it.value for it in cls)

    type: FieldMappingType
    value: str
    field_name: str

    def __init__(self, field_type: FieldMappingType, field_name: str, value: str):
        self.type = field_type
        self.field_name = field_name.lower()

        if self.type == FieldMapping.FieldMappingType.COLUMN:
            self.value = value.lower()
        else:
            self.value = value


class NoteModelMapping(YamlFile):
    config_entry = {}
    expected_keys = {
        NoteModelMappingKeys.NOTE_MODEL.value: ConfigKey(True, str, None),
        NoteModelMappingKeys.COLUMNS.value: ConfigKey(True, dict, None),
        NoteModelMappingKeys.PERSONAL_FIELDS.value: ConfigKey(False, list, None),
    }
    subconfig_filter = None

    note_model: DeckPartNoteModel
    columns: List[FieldMapping]
    personal_fields: List[FieldMapping]

    required_fields_definitions = [DeckPartNoteKeys.GUID.value, DeckPartNoteKeys.TAGS.value]

    def __init__(self, config_data: dict, read_now=True):
        self.setup_config_with_subconfig_replacement(config_data)
        self.verify_config_entry()

        columns = self.get_config(NoteModelMappingKeys.COLUMNS)
        personal_fields = self.get_config(NoteModelMappingKeys.PERSONAL_FIELDS, [])

        self.columns = [FieldMapping(
                                field_type=FieldMapping.FieldMappingType.COLUMN,
                                field_name=field,
                                value=columns[field]) for field in columns]

        self.personal_fields = [FieldMapping(
                                field_type=FieldMapping.FieldMappingType.PERSONAL_FIELD,
                                field_name=field,
                                value="") for field in personal_fields]

        self.note_model = DeckPartNoteModel.create(self.get_config(NoteModelMappingKeys.NOTE_MODEL), read_now=read_now)




