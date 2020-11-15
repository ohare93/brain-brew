from dataclasses import dataclass
from typing import Optional, Union

from brain_brew.representation.build_config.build_task import DeckPartBuildTask
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiJsonWrapper
from brain_brew.representation.transformers.base_deck_part_from import BaseDeckPartsFrom
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.headers_repr import Headers
from brain_brew.representation.json.wrappers_for_crowd_anki import CA_NOTE_MODELS, CA_NOTES, CA_MEDIA_FILES,\
    CA_CHILDREN, CA_TYPE


headers_skip_keys = [CA_NOTE_MODELS, CA_NOTES, CA_MEDIA_FILES]
headers_default_values = {
    CA_TYPE: "Deck",
    CA_CHILDREN: [],
}


@dataclass
class HeadersFromCrowdAnki(BaseDeckPartsFrom, DeckPartBuildTask):
    @classmethod
    def task_regex(cls) -> str:
        return r'headers_from_crowd_anki'

    @classmethod
    def yamale_validator_and_deps(cls) -> (str, set):
        return f'''\
            {cls.task_regex()}:
              source: str()
              part_id: str()
              save_to_file: str(required=False)
        '''

    @dataclass
    class Representation(BaseDeckPartsFrom.Representation):
        source: str

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            ca_export=CrowdAnkiExport.create_or_get(rep.source),
            part_id=rep.part_id,
            save_to_file=rep.save_to_file
        )

    ca_export: CrowdAnkiExport

    def execute(self):
        ca_wrapper: CrowdAnkiJsonWrapper = self.ca_export.json_data

        headers = Headers(self.crowd_anki_to_headers(ca_wrapper.data))

        DeckPartHolder.override_or_create(self.part_id, self.save_to_file, headers)

    @staticmethod
    def crowd_anki_to_headers(ca_data: dict):
        return {key: value for key, value in ca_data.items()
                if key not in headers_skip_keys and key not in headers_default_values.keys()}
