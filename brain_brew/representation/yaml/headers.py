from dataclasses import dataclass

from brain_brew.representation.json.wrappers_for_crowd_anki import CA_NAME, CA_DESCRIPTION, CA_UUID
from brain_brew.representation.yaml.yaml_object import YamlObject


@dataclass
class Headers(YamlObject):
    data: dict

    @classmethod
    def from_yaml_file(cls, filename: str):
        return cls(data=cls.read_to_dict(filename))

    def encode(self) -> dict:
        return self.data

    @property
    def name(self) -> str:
        return self.data[CA_NAME]

    @name.setter
    def name(self, desc: str):
        self.data[CA_NAME] = desc

    @property
    def description(self) -> str:
        return self.data.get(CA_DESCRIPTION, "")

    @description.setter
    def description(self, desc: str):
        self.data[CA_DESCRIPTION] = desc

    @property
    def crowdanki_uuid(self) -> str:
        return self.data.get(CA_UUID, "")

    @crowdanki_uuid.setter
    def crowdanki_uuid(self, desc: str):
        self.data[CA_UUID] = desc

    @property
    def data_without_name(self) -> dict:
        return {k: v for k, v in self.data.items() if k != CA_NAME}
