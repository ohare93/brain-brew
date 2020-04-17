# import logging
# from typing import List
#
# from brain_brew.build_tasks.build_task_generic import BuildTaskGeneric
# from brain_brew.constants.build_config_keys import BuildTaskEnum, BuildConfigKeys
# from brain_brew.build_tasks.source_csv import SourceCsv
# from brain_brew.representation.configuration.yaml_file import YamlFile, ConfigKey
# from brain_brew.representation.json.deck_part_notes import DeckPartNotes
#
#
# class SourceCsvCollection(YamlFile, BuildTaskGeneric):
#     @staticmethod
#     def get_build_keys():
#         return [
#             BuildTaskEnum("deck_parts_to_csv_collection", SourceCsvCollection,
#                           "deck_parts_to_source", "source_to_deck_parts"),
#             BuildTaskEnum("csv_collection_to_deck_parts", SourceCsvCollection, "source_to_deck_parts",
#                           "deck_parts_to_source"),
#         ]
#
#     config_entry = {}
#     expected_keys = {
#         BuildConfigKeys.NOTES.value: ConfigKey(True, str, None),
#         BuildConfigKeys.SUBCONFIG.value: ConfigKey(True, list, None)
#     }
#     subconfig_filter = None
#
#     source_csvs: List[SourceCsv]
#     notes: DeckPartNotes
#
#     def __init__(self, config_data: dict, read_now=True):
#         self.setup_config_with_subconfig_replacement(config_data)
#         self.verify_config_entry()
#
#         notes_filename = self.config_entry[BuildConfigKeys.NOTES.value]
#         self.notes = DeckPartNotes.create(notes_filename, read_now=read_now)
#
#         self.source_csvs = []
#         for csv_entry in self.config_entry[BuildConfigKeys.SUBCONFIG.value]:
#             if BuildConfigKeys.NOTES.value in csv_entry.keys():
#                 if csv_entry[BuildConfigKeys.NOTES.value] != notes_filename:
#                     logging.warning(f"Csv config referenced in SourceCsvCollection already contains "
#                                     f"{BuildConfigKeys.NOTES.value} value {csv_entry[BuildConfigKeys.NOTES.value ]}. "
#                                     f"It was overwritten with {notes_filename}")
#
#             csv_entry[BuildConfigKeys.NOTES.value] = notes_filename
#             self.source_csvs.append(SourceCsv(csv_entry, read_now=read_now))
#
#     def source_to_deck_parts(self):
#         source_data = []
#         for csv in self.source_csvs:
#             notes_data = csv.notes_to_deck_parts()
#
#             source_data += notes_data
#
#         self.notes.set_data(source_data)
#
#     def deck_parts_to_source(self):
#         for csv in self.source_csvs:  # TODO: Make this gather the data and spread it out to the right files
#             csv.deck_parts_to_source()
