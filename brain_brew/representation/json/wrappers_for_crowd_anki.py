from typing import List


CA_NOTE_MODELS = "note_models"
CA_NOTES = "notes"
CA_MEDIA_FILES = "media_files"

NOTE_MODEL = "note_model_uuid"
FLAGS = "flags"
GUID = "guid"
TAGS = "tags"
FIELDS = "fields"


class CrowdAnkiJsonWrapper:
    data: dict

    def __init__(self, data: dict = None):
        self.data = data

    @property
    def note_models(self) -> str:
        return self.data[CA_NOTE_MODELS]

    @note_models.setter
    def note_models(self, value: str):
        self.data.setdefault(CA_NOTE_MODELS, value)

    @property
    def notes(self) -> str:
        return self.data[CA_NOTES]

    @notes.setter
    def notes(self, value: str):
        self.data.setdefault(CA_NOTES, value)

    @property
    def media_files(self) -> str:
        return self.data[CA_MEDIA_FILES]

    @media_files.setter
    def media_files(self, value: str):
        self.data.setdefault(CA_MEDIA_FILES, value)


class CrowdAnkiNoteWrapper:
    data: dict

    def __init__(self, data: dict = None):
        self.data = data

    @property
    def note_model(self) -> str:
        return self.data[NOTE_MODEL]

    @note_model.setter
    def note_model(self, value: str):
        self.data.setdefault(NOTE_MODEL, value)

    @property
    def flags(self) -> int:
        return self.data[FLAGS]

    @flags.setter
    def flags(self, value: int):
        self.data.setdefault(FLAGS, value)

    @property
    def guid(self) -> str:
        return self.data[GUID]

    @guid.setter
    def guid(self, value: str):
        self.data.setdefault(GUID, value)

    @property
    def tags(self) -> list:
        return self.data[TAGS]

    @tags.setter
    def tags(self, value: list):
        self.data.setdefault(TAGS, value)

    @property
    def fields(self) -> List[str]:
        return self.data[FIELDS]

    @fields.setter
    def fields(self, value: List[str]):
        self.data.setdefault(FIELDS, value)
