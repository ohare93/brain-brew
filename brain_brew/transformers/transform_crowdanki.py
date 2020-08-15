from dataclasses import dataclass
from typing import List, Optional, Union

# from brain_brew.representation.json.crowd_anki_export import CrowdAnkiExport
# from brain_brew.representation.transformers.generic_to_deck_part import TrGenericToDeckPart
#
#
# @dataclass
# class TrCrowdAnkiToNotes(TrGenericToDeckPart):
#     @dataclass
#     class Representation:
#         file: str
#         sort_order: Optional[Union[str, List[str]]]
#         media: Optional[bool]
#         useless_note_keys: Optional[Union[dict, list]]
#
#     crowdanki_file: CrowdAnkiExport
#     sort_order: Optional[List[str]]
#     media: bool
#     useless_note_keys: list
#
#
# @dataclass
# class TrNotesToCrowdAnki(TrNotesToGeneric):
#     @dataclass
#     class Representation:
#         file: str
#         sort_order: Optional[Union[str, List[str]]]
#         media: Optional[bool]
#         useless_note_keys: Optional[dict]  # TODO: use default value
#
#     sort_order: Optional[List[str]]
#     media: bool
#     useless_note_keys: dict
#
