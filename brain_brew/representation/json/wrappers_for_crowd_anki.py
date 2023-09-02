from typing import List


CA_NOTE_MODELS = "note_models"
CA_NOTES = "notes"
CA_MEDIA_FILES = "media_files"
CA_CHILDREN = "children"
CA_TYPE = "__type__"
CA_NAME = "name"
CA_DESCRIPTION = "desc"
CA_UUID = "crowdanki_uuid"

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
    def children(self) -> list:
        return self.data.get(CA_CHILDREN, [])

    @property
    def note_models(self) -> list:
        return CrowdAnkiJsonWrapper.get_from_self_and_children_recursively(self.data, [], CA_NOTE_MODELS)


    @note_models.setter
    def note_models(self, value: list):
        self.data[CA_NOTE_MODELS] = value

    @property
    def notes(self) -> list:
        return CrowdAnkiJsonWrapper.get_from_self_and_children_recursively(self.data, [], CA_NOTES)

    @notes.setter
    def notes(self, value: list):
        self.data[CA_NOTES] = value

    @property
    def media_files(self) -> list:
        return CrowdAnkiJsonWrapper.get_from_self_and_children_recursively(self.data, [], CA_MEDIA_FILES)

    @media_files.setter
    def media_files(self, value: list):
        self.data[CA_MEDIA_FILES] = value

    @property
    def name(self) -> list:
        return self.data.get(CA_NAME, [])

    @name.setter
    def name(self, value: list):
        self.data[CA_NAME] = value

    @staticmethod
    def get_from_self_and_children_recursively(data: dict, running_data: list, key_name: str):
        running_data += data.get(key_name, [])
        children = data.get(CA_CHILDREN, [])
        if isinstance(children, list):
            for child in children:
                running_data = CrowdAnkiJsonWrapper.get_from_self_and_children_recursively(child, running_data, key_name)
        return running_data


class CrowdAnkiNoteWrapper:
    data: dict

    def __init__(self, data: dict = None):
        self.data = data

    @property
    def note_model(self) -> str:
        return self.data.get(NOTE_MODEL)

    @note_model.setter
    def note_model(self, value: str):
        self.data[NOTE_MODEL] = value

    @property
    def flags(self) -> int:
        return self.data.get(FLAGS)

    @flags.setter
    def flags(self, value: int):
        self.data[FLAGS] = value

    @property
    def guid(self) -> str:
        return self.data.get(GUID)

    @guid.setter
    def guid(self, value: str):
        self.data[GUID] = value

    @property
    def tags(self) -> list:
        return self.data.get(TAGS, [])

    @tags.setter
    def tags(self, value: list):
        self.data[TAGS] = value

    @property
    def fields(self) -> List[str]:
        return self.data.get(FIELDS, [])

    @fields.setter
    def fields(self, value: List[str]):
        self.data[FIELDS] = value
