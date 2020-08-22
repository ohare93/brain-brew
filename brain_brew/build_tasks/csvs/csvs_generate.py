from dataclasses import dataclass
from typing import List, Dict, Union

from brain_brew.build_tasks.csvs.shared_base_csvs import SharedBaseCsvs
from brain_brew.representation.build_config.build_task import TopLevelBuildTask
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.note_repr import Notes, Note
from brain_brew.transformers.transform_csv_collection import TransformCsvCollection
from brain_brew.utils import all_combos_prepend_append


@dataclass
class CsvsGenerate(SharedBaseCsvs, TopLevelBuildTask):
    task_names = all_combos_prepend_append(["Csv Collection", "Csv"], "Generate ", "s")

    notes: DeckPartHolder[Notes]

    @dataclass(init=False)
    class Representation(SharedBaseCsvs.Representation):
        notes: str

        def __init__(self, notes, file_mappings, note_model_mappings):
            SharedBaseCsvs.Representation.__init__(self, file_mappings, note_model_mappings)
            self.notes = notes

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            notes=DeckPartHolder.from_deck_part_pool(rep.notes),
            file_mappings=rep.get_file_mappings(),
            note_model_mappings=rep.get_note_model_mappings()
        )

    def execute(self):
        notes: List[Note] = self.notes.deck_part.get_notes()
        self.verify_notes_match_note_model_mappings(notes)

        csv_data: Dict[str, dict] = TransformCsvCollection.notes_to_csv_collection(notes, self.note_model_mappings)

        # TODO: Dry run option, to not save anything at this stage

        for fm in self.file_mappings:
            fm.compile_data()
            fm.set_relevant_data(csv_data)
            fm.write_file_on_close()

    def verify_notes_match_note_model_mappings(self, notes: List[Note]):
        note_models_used = {note.note_model for note in notes}
        errors = [TypeError(f"Unknown note model type '{model}' in deck part '{self.notes.name}'. "
                            f"Add mapping for that model.")
                  for model in note_models_used if model not in self.note_model_mappings.keys()]

        if errors:
            raise Exception(errors)
