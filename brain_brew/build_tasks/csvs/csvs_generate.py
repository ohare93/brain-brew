from dataclasses import dataclass, field
from typing import List, Dict, Union

from brain_brew.build_tasks.csvs.shared_base_csvs import SharedBaseCsvs
from brain_brew.configuration.build_config.build_task import TopLevelBuildTask
from brain_brew.configuration.part_holder import PartHolder
from brain_brew.representation.yaml.notes import Notes, Note
from brain_brew.transformers.file_mapping import FileMapping
from brain_brew.transformers.note_model_mapping import NoteModelMapping
from brain_brew.utils import join_tags


@dataclass
class CsvsGenerate(SharedBaseCsvs, TopLevelBuildTask):
    @classmethod
    def task_name(cls) -> str:
        return r'generate_csvs'

    @classmethod
    def task_regex(cls) -> str:
        return r'generate_csv[s]?'

    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            notes: str()
            note_model_mappings: list(include('{NoteModelMapping.task_name()}'))
            file_mappings: list(include('{FileMapping.task_name()}'))
        '''

    @classmethod
    def yamale_dependencies(cls) -> set:
        return {NoteModelMapping, FileMapping}

    notes: PartHolder[Notes]  # TODO: Accept Multiple Note Parts

    @dataclass
    class Representation(SharedBaseCsvs.Representation):
        notes: str

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            notes=PartHolder.from_file_manager(rep.notes),
            file_mappings=rep.get_file_mappings(),
            note_model_mappings=dict(*map(cls.map_nmm, rep.note_model_mappings))
        )

    def execute(self):
        self.verify_contents()

        notes: List[Note] = self.notes.part.get_sorted_notes_copy(
            sort_by_keys=[], reverse_sort=False, case_insensitive_sort=False)
        self.verify_notes_match_note_model_mappings(notes)

        csv_data: List[dict] = [self.note_to_csv_row(note, self.note_model_mappings) for note in notes]
        rows_by_guid = {row["guid"]: row for row in csv_data}

        for fm in self.file_mappings:
            fm.compile_data()
            fm.set_relevant_data(rows_by_guid)
            fm.write_file_on_close()

    def verify_notes_match_note_model_mappings(self, notes: List[Note]):
        note_models_used = {note.note_model for note in notes}
        errors = [TypeError(f"Unknown note model type '{model}' in deck part '{self.notes.part_id}'. "
                            f"Add mapping for that model.")
                  for model in note_models_used if model not in self.note_model_mappings.keys()]

        if errors:
            raise Exception(errors)

    @staticmethod
    def note_to_csv_row(note: Note, note_model_mappings: Dict[str, NoteModelMapping]) -> dict:
        nm_name = note.note_model
        row = note_model_mappings[nm_name].note_models[nm_name].part.zip_field_to_data(note.fields)
        row["guid"] = note.guid
        row["tags"] = join_tags(note.tags)
        # TODO: Flags?

        return note_model_mappings[nm_name].note_fields_map_to_csv_row(row)
