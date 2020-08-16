from dataclasses import dataclass
from typing import Optional, Union, List

from brain_brew.build_tasks.crowd_anki.shared_base_notes import SharedBaseNotes
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.transformers.transform_crowdanki import TransformCrowdAnki


@dataclass
class TrCrowdAnkiToNotes(SharedBaseNotes):
    @dataclass
    class Representation(SharedBaseNotes.Representation):
        pass

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            name=rep.name,
            sort_order=SharedBaseNotes._get_sort_order(rep.sort_order),
            move_media=SharedBaseNotes._get_move_media(rep.move_media),
            useless_note_keys=SharedBaseNotes._get_useless_note_keys(rep.useless_note_keys)
        )

    def execute(self, crowd_anki_export: CrowdAnkiExport):
        ca_wrapper = crowd_anki_export.read_json_file()

        TransformCrowdAnki.crowd_anki_to_notes(ca_wrapper.notes, )