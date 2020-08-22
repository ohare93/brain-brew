from typing import List, Dict


from brain_brew.representation.configuration.note_model_mapping import NoteModelMapping
from brain_brew.representation.yaml.note_repr import Note
from brain_brew.transformers.base_transform_notes import TrNotes


class TransformCsvCollection(TrNotes):
    @classmethod
    def notes_to_csv_collection(cls, notes: List[Note], note_model_mappings: Dict[str, NoteModelMapping]) -> Dict[str, dict]:
        csv_data: Dict[str, dict] = {}
        for note in notes:
            nm_name = note.note_model
            row = note_model_mappings[nm_name].note_models[nm_name].deck_part.zip_field_to_data(note.fields)
            row["guid"] = note.guid
            row["tags"] = cls.join_tags(note.tags)

            formatted_row = note_model_mappings[nm_name].note_fields_map_to_csv_row(row)  # TODO: Do not edit data, make copy

            csv_data.setdefault(row["guid"], formatted_row)

        return csv_data

    @classmethod
    def csv_collection_to_notes(cls, csv_rows: List[dict], note_model_mappings: Dict[str, NoteModelMapping]) -> List[Note]:
        deck_part_notes: List[Note] = []

        # Get Guid, Tags, NoteTypeName, Fields
        for row in csv_rows:
            note_model_name = row["note_model"]  # TODO: Use object
            row_nm: NoteModelMapping = note_model_mappings[note_model_name]

            filtered_fields = row_nm.csv_row_map_to_note_fields(row)

            guid = filtered_fields.pop("guid")
            tags = cls.split_tags(filtered_fields.pop("tags"))
            flags = filtered_fields.pop("flags") if "flags" in filtered_fields else 0

            fields = row_nm.field_values_in_note_model_order(note_model_name, filtered_fields)

            deck_part_notes.append(Note(guid=guid, tags=tags, note_model=note_model_name, fields=fields, flags=flags))

        return deck_part_notes
