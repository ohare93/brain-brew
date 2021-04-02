from dataclasses import dataclass, field
from typing import Union, Optional

from brain_brew.commands.run_recipe.build_task import BuildPartTask
from brain_brew.configuration.part_holder import PartHolder
from brain_brew.configuration.representation_base import RepresentationBase
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.representation.json.wrappers_for_crowd_anki import CA_NOTE_MODELS, CA_NOTES, CA_MEDIA_FILES, \
    CA_CHILDREN, CA_TYPE
from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiJsonWrapper
from brain_brew.representation.yaml.headers import Headers

headers_skip_keys = [CA_NOTE_MODELS, CA_NOTES, CA_MEDIA_FILES]
headers_default_values = {
    CA_TYPE: "Deck",
    CA_CHILDREN: [],
}


@dataclass
class HeadersFromCrowdAnki(BuildPartTask):
    @classmethod
    def task_name(cls) -> str:
        return r'headers_from_crowd_anki'

    @classmethod
    def task_regex(cls) -> str:
        return r'header[s]?_from_crowd_anki'

    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            source: str()
            part_id: str()
            save_to_file: str(required=False)
        '''

    @dataclass
    class Representation(RepresentationBase):
        source: str
        part_id: str
        save_to_file: Optional[str] = field(default=None)

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            rep=rep,
            ca_export=CrowdAnkiExport.create_or_get(rep.source),
            part_id=rep.part_id,
            save_to_file=rep.save_to_file
        )

    rep: Representation
    ca_export: CrowdAnkiExport
    part_id: str
    save_to_file: Optional[str]

    def execute(self):
        ca_wrapper: CrowdAnkiJsonWrapper = self.ca_export.json_data

        headers = Headers(self.crowd_anki_to_headers(ca_wrapper.data))

        return PartHolder.override_or_create(self.part_id, self.save_to_file, headers)

    @staticmethod
    def crowd_anki_to_headers(ca_data: dict):
        return {key: value for key, value in ca_data.items()
                if key not in headers_skip_keys and key not in headers_default_values.keys()}
