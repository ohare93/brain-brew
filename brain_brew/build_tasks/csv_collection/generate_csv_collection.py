from dataclasses import dataclass
from typing import List, Dict

from brain_brew.build_tasks.csv_collection.config.csv_collection_shared import CsvCollectionShared
from brain_brew.representation.build_config.build_task import TopLevelBuildTask
from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder
from brain_brew.representation.yaml.note_repr import Notes, Note
from brain_brew.transformers.transform_csv_collection import TransformCsvCollection
from brain_brew.utils import all_combos_prepend_append


@dataclass
class GenerateCsvCollection(CsvCollectionShared, TopLevelBuildTask):
    task_names = all_combos_prepend_append(["Csv Collection", "Csv"], "Generate ", "s")

    notes: DeckPartHolder[Notes]

    @dataclass(init=False)
    class Representation(CsvCollectionShared.Representation):
        notes: str

        def __init__(self, notes, file_mappings, note_model_mappings):
            CsvCollectionShared.Representation.__init__(self, file_mappings, note_model_mappings)
            self.notes = notes

        @classmethod
        def from_dict(cls, data: dict):
            return cls(**data)

    @classmethod
    def from_repr(cls, data: Representation):
        return cls(
            notes=DeckPartHolder.from_deck_part_pool(data.notes),
            file_mappings=data.get_file_mappings(),
            note_model_mappings=data.get_note_model_mappings()
        )

    @classmethod
    def from_dict(cls, data: dict):
        return cls.from_repr(cls.Representation.from_dict(data))

    def execute(self):
        notes: List[Note] = self.notes.deck_part.get_notes()
        self.verify_notes_match_note_model_mappings(notes)

        csv_data: Dict[str, dict] = TransformCsvCollection.notes_to_csv_collection(notes, self.note_model_mappings)

        # TODO: Dry run option, to not save anything at this stage

        for fm in self.file_mappings:
            fm.compile_data()
            fm.set_relevant_data(csv_data)

    def verify_notes_match_note_model_mappings(self, notes: List[Note]):
        note_models_used = {note.note_model for note in notes}
        errors = [TypeError(f"Unknown note model type '{model}' in deck part '{self.notes.name}'. "
                            f"Add mapping for that model.")
                  for model in note_models_used if model not in self.note_model_mappings.keys()]

        if errors:
            raise Exception(errors)