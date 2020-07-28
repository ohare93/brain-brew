from dataclasses import dataclass
from typing import List, Dict

from brain_brew.representation.build_config.build_task import TopLevelBuildTask, GenerateDeckPartBuildTask
from brain_brew.representation.configuration.csv_file_mapping import CsvFileMapping
from brain_brew.representation.configuration.note_model_mapping import NoteModelMapping
from brain_brew.representation.deck_part_transformers.tr_notes_generic import TrNotesToGeneric, TrGenericToNotes
from brain_brew.representation.yaml.note_repr import DeckPartNotes, Note


@dataclass
class TrCsvCollectionShared:
    @dataclass(init=False)
    class Representation:
        file_mappings: List[CsvFileMapping.Representation]
        note_model_mappings: List[NoteModelMapping.Representation]

        def __init__(self, file_mappings, note_model_mappings):
            self.file_mappings = list(map(CsvFileMapping.Representation.from_dict, file_mappings))
            self.note_model_mappings = list(map(NoteModelMapping.Representation.from_dict, note_model_mappings))

        def get_file_mappings(self) -> List[CsvFileMapping]:
            return list(map(CsvFileMapping.from_repr, self.file_mappings))

        def get_note_model_mappings(self) -> Dict[str, NoteModelMapping]:
            def map_nmm(nmm_to_map: str):
                nmm = NoteModelMapping.from_repr(nmm_to_map)
                return nmm.get_note_model_mapping_dict()

            return dict(*map(map_nmm, self.note_model_mappings))

    file_mappings: List[CsvFileMapping]
    note_model_mappings: Dict[str, NoteModelMapping]

    def verify_contents(self):
        errors = []

        for nm in self.note_model_mappings.values():
            try:
                nm.verify_contents()
            except KeyError as e:
                errors.append(e)

        for fm in self.file_mappings:
            # Check all necessary key values are present
            try:
                fm.verify_contents()
            except KeyError as e:
                errors.append(e)

            # Check all referenced note models have a mapping
            for csv_map in self.file_mappings:
                for nm in csv_map.get_used_note_model_names():
                    if nm not in self.note_model_mappings.keys():
                        errors.append(f"Missing Note Model Map for {nm}")

        # Check each of the Csvs (or their derivatives) contain all the necessary columns for their stated note model
        # for cfm in self.file_mappings:
        #     note_model_names = cfm.get_used_note_model_names()
        #     available_columns = cfm.get_available_columns()
        #
        #     referenced_note_models_maps = [value for key, value in self.note_model_mappings.items() if
        #                                    key in note_model_names]
        #     for nm_map in referenced_note_models_maps:
        #         missing_columns = [col for col in nm_map.note_model.fields_lowercase if
        #                            col not in nm_map.csv_headers_map_to_note_fields(available_columns)]
        #         if missing_columns:
        #             errors.append(KeyError(f"Csvs are missing columns from {nm_map.note_model.name}", missing_columns))

        if errors:
            raise Exception(errors)


@dataclass
class TrCsvCollectionToNotes(GenerateDeckPartBuildTask, TrCsvCollectionShared, TrGenericToNotes):
    task_names = ["Notes From Csv Collection", "Notes From Csv", "Notes From Csvs"]

    @dataclass(init=False)
    class Representation(TrCsvCollectionShared.Representation, TrGenericToNotes.Representation):
        def __init__(self, name, file_mappings, note_model_mappings, save_to_file=None):
            TrCsvCollectionShared.Representation.__init__(self, file_mappings, note_model_mappings)
            TrGenericToNotes.Representation.__init__(self, name, save_to_file)

        @classmethod
        def from_dict(cls, data: dict):
            return cls(**data)

    @classmethod
    def from_repr(cls, data: Representation):
        return cls(
            name=data.name,
            save_to_file=data.save_to_file,
            file_mappings=data.get_file_mappings(),
            note_model_mappings=data.get_note_model_mappings()
        )

    def __repr__(self):
        return f'TrCsvCollectionToNotes({self.name!r}, {self.save_to_file!r}, {self.file!r}, {self.note_model_mappings!r}, '

    @classmethod
    def from_dict(cls, data: dict):
        return cls.from_repr(TrCsvCollectionToNotes.Representation.from_dict(data))

    def execute(self):
        csv_data_by_guid: Dict[str, dict] = {}
        for csv_map in self.file_mappings:
            csv_map.compile_data()
            csv_data_by_guid = {**csv_data_by_guid, **csv_map.compiled_data}
        csv_rows: List[dict] = list(csv_data_by_guid.values())

        deck_part_notes: List[Note] = []

        # Get Guid, Tags, NoteTypeName, Fields
        for row in csv_rows:
            note_model_name = row["note_model"]  # TODO: Use object
            row_nm: NoteModelMapping = self.note_model_mappings[note_model_name]

            filtered_fields = row_nm.csv_row_map_to_note_fields(row)

            guid = filtered_fields.pop("guid")
            tags = self.split_tags(filtered_fields.pop("tags"))

            fields = row_nm.field_values_in_note_model_order(note_model_name, filtered_fields)

            deck_part_notes.append(Note(guid=guid, tags=tags, note_model=note_model_name, fields=fields))

        DeckPartNotes.from_list_of_notes(self.name, self.save_to_file, deck_part_notes)
        # TODO: Save to the singleton holder


@dataclass
class TrNotesToCsvCollection(TopLevelBuildTask, TrCsvCollectionShared, TrNotesToGeneric):
    task_names = ["Generate Csv Collection", "Generate Csv Collections", "Generate Csv", "Generate Csvs"]

    @dataclass(init=False)
    class Representation(TrCsvCollectionShared.Representation, TrNotesToGeneric.Representation):
        def __init__(self, notes, file_mappings, note_model_mappings):
            TrCsvCollectionShared.Representation.__init__(self, file_mappings, note_model_mappings)
            TrNotesToGeneric.Representation.__init__(self, notes)

        @classmethod
        def from_dict(cls, data: dict):
            return cls(**data)

    @classmethod
    def from_repr(cls, data: Representation):
        return cls(
            notes=DeckPartNotes.from_deck_part_pool(data.notes),
            file_mappings=data.get_file_mappings(),
            note_model_mappings=data.get_note_model_mappings()
        )

    @classmethod
    def from_dict(cls, data: dict):
        return cls.from_repr(TrNotesToCsvCollection.Representation.from_dict(data))

    def execute(self):
        notes_data = self.notes.get_notes()
        self.verify_notes_match_note_model_mappings(notes_data)

        csv_data: Dict[str, dict] = {}
        for note in notes_data:
            nm_name = note.note_model
            row = self.note_model_mappings[nm_name].note_models[nm_name].zip_field_to_data(note.fields)
            row["guid"] = note.guid
            row["tags"] = self.join_tags(note.tags)

            formatted_row = self.note_model_mappings[nm_name].note_fields_map_to_csv_row(row)  # TODO: Do not edit data, make copy

            csv_data.setdefault(row["guid"], formatted_row)

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
