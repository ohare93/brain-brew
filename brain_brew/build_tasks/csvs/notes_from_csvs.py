from dataclasses import dataclass, field
from typing import Dict, List, Union, Optional

from brain_brew.build_tasks.csvs.shared_base_csvs import SharedBaseCsvs
from brain_brew.representation.build_config.build_task import BuildPartTask
from brain_brew.representation.configuration.csv_file_mapping import FileMapping
from brain_brew.representation.configuration.note_model_mapping import NoteModelMapping
from brain_brew.representation.yaml.note_repr import Note, Notes
from brain_brew.representation.yaml.part_holder import PartHolder
from brain_brew.utils import split_tags


@dataclass
class NotesFromCsvs(SharedBaseCsvs, BuildPartTask):
    @classmethod
    def task_name(cls) -> str:
        return r'notes_from_csvs'

    @classmethod
    def task_regex(cls) -> str:
        return r'notes_from_csv[s]?'

    @classmethod
    def yamale_schema(cls) -> str:
        return f'''\
            part_id: str()
            save_to_file: str(required=False)
            note_model_mappings: list(include('{NoteModelMapping.task_name()}'))
            file_mappings: list(include('{FileMapping.task_name()}'))
        '''

    @classmethod
    def yamale_dependencies(cls) -> set:
        return {NoteModelMapping, FileMapping}

    @dataclass
    class Representation(SharedBaseCsvs.Representation):
        part_id: str
        save_to_file: Optional[str] = field(default=None)

    @classmethod
    def from_repr(cls, data: Union[Representation, dict]):
        rep: cls.Representation = data if isinstance(data, cls.Representation) else cls.Representation.from_dict(data)
        return cls(
            part_id=rep.part_id,
            save_to_file=rep.save_to_file,
            file_mappings=rep.get_file_mappings(),
            note_model_mappings_to_read=rep.note_model_mappings
        )

    part_id: str
    save_to_file: Optional[str]

    def execute(self):
        self.setup_note_model_mappings()
        self.verify_contents()

        csv_data_by_guid: Dict[str, dict] = {}
        for csv_map in self.file_mappings:
            csv_map.compile_data()
            csv_data_by_guid = {**csv_data_by_guid, **csv_map.compiled_data}
        csv_rows: List[dict] = list(csv_data_by_guid.values())

        notes_part: List[Note] = [self.csv_row_to_note(row, self.note_model_mappings) for row in csv_rows]

        notes = Notes.from_list_of_notes(notes_part)
        PartHolder.override_or_create(self.part_id, self.save_to_file, notes)

    @staticmethod
    def csv_row_to_note(row: dict, note_model_mappings: Dict[str, NoteModelMapping]) -> Note:
        note_model_name = row["note_model"]  # TODO: Use object
        row_nm: NoteModelMapping = note_model_mappings[note_model_name]

        filtered_fields = row_nm.csv_row_map_to_note_fields(row)

        guid = filtered_fields.pop("guid")
        tags = split_tags(filtered_fields.pop("tags"))
        flags = filtered_fields.pop("flags") if "flags" in filtered_fields else 0

        fields = row_nm.field_values_in_note_model_order(note_model_name, filtered_fields)

        return Note(guid=guid, tags=tags, note_model=note_model_name, fields=fields, flags=flags)
