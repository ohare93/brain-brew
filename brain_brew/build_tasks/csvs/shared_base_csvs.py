from dataclasses import dataclass, field
from typing import List, Dict

from brain_brew.representation.build_config.representation_base import RepresentationBase
from brain_brew.representation.configuration.csv_file_mapping import FileMapping
from brain_brew.representation.configuration.note_model_mapping import NoteModelMapping


@dataclass
class SharedBaseCsvs:
    @dataclass(init=False)
    class Representation(RepresentationBase):
        file_mappings: List[FileMapping.Representation]
        note_model_mappings: List[NoteModelMapping.Representation]

        def __init__(self, file_mappings, note_model_mappings):
            self.file_mappings = list(map(FileMapping.Representation.from_dict, file_mappings))
            self.note_model_mappings = list(map(NoteModelMapping.Representation.from_dict, note_model_mappings))

        def get_file_mappings(self) -> List[FileMapping]:
            return list(map(FileMapping.from_repr, self.file_mappings))

    file_mappings: List[FileMapping]
    note_model_mappings_representations: List[NoteModelMapping.Representation]
    note_model_mappings: Dict[str, NoteModelMapping] = field(default_factory=dict)

    def get_note_model_mappings(self):
        def map_nmm(nmm_to_map: str):
            nmm = NoteModelMapping.from_repr(nmm_to_map)
            return nmm.get_note_model_mapping_dict()

        self.note_model_mappings = dict(*map(map_nmm, self.note_model_mappings_representations))

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
        for cfm in self.file_mappings:
            note_model_names = cfm.get_used_note_model_names()
            available_columns = cfm.get_available_columns()

            referenced_note_models_maps = [value for key, value in self.note_model_mappings.items() if
                                           key in note_model_names]
            for nm_map in referenced_note_models_maps:
                for holder in nm_map.note_models.values():
                    missing_columns = [col for col in holder.deck_part.field_names_lowercase if
                                       col not in nm_map.csv_headers_map_to_note_fields(available_columns)]
                    if missing_columns:
                        errors.append(KeyError(f"Csvs are missing columns from {holder.name}", missing_columns))

        if errors:
            raise Exception(errors)
