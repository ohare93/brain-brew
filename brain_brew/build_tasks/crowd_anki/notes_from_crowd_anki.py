from dataclasses import dataclass
from typing import Optional, Union, List

from brain_brew.build_tasks.crowd_anki.shared_base_notes import SharedBaseNotes
from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
from brain_brew.representation.json.wrappers_for_crowd_anki import CrowdAnkiJsonWrapper
from brain_brew.representation.transformers.base_deck_part_from import BaseDeckPartsFrom
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.note_repr import Notes
from brain_brew.transformers.transform_crowdanki import TransformCrowdAnki


@dataclass
class TrCrowdAnkiToNotes(SharedBaseNotes, BaseDeckPartsFrom):
    @dataclass
    class Representation(SharedBaseNotes.Representation, BaseDeckPartsFrom.Representation):
        pass

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            name=rep.name,
            sort_order=SharedBaseNotes._get_sort_order(rep.sort_order),
            save_to_file=rep.save_to_file
        )

    def execute(self, ca_wrapper: CrowdAnkiJsonWrapper, nm_id_to_name: dict) -> Notes:
        note_list = TransformCrowdAnki.crowd_anki_to_notes(ca_wrapper.notes, nm_id_to_name)

        notes = Notes.from_list_of_notes(note_list)  # TODO: pass in sort method

        DeckPartHolder.override_or_create(self.name, self.save_to_file, notes)

        return notes
