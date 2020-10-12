from dataclasses import dataclass
from typing import Dict, Type, List, Tuple
from textwrap import dedent

from brain_brew.representation.build_config.build_task import BuildTask, TopLevelBuildTask, DeckPartBuildTask
from brain_brew.representation.build_config.recipe_builder import RecipeBuilder

# Build Tasks
from brain_brew.build_tasks.deck_parts.from_yaml_deck_part import NotesFromYamlDeckPart, HeadersFromYamlDeckPart, NoteModelsFromYamlDeckPart  # noqa
from brain_brew.build_tasks.csvs.notes_from_csvs import NotesFromCsvs  # noqa
from brain_brew.build_tasks.crowd_anki.crowd_anki_to_deck_parts import CrowdAnkiToDeckParts  # noqa


@dataclass
class BuildDeckParts(RecipeBuilder, TopLevelBuildTask):
    @classmethod
    def task_regex(cls) -> str:
        return r'build_deck_parts'

    @classmethod
    def known_task_dict(cls) -> Dict[str, Type[BuildTask]]:
        return DeckPartBuildTask.get_all_task_regex()

    @classmethod
    def known_validators(cls) -> List[Tuple[str, set]]:
        return DeckPartBuildTask.get_all_validators()

    @classmethod
    def from_repr(cls, data: List[dict]):
        if not isinstance(data, list):
            raise TypeError(f"GenerateDeckParts needs a list")
        return cls.from_list(data)

    @classmethod
    def yamale_validator_and_deps(cls) -> (str, set):
        # list(
        #     map(include('from_deck_parts'), key=regex('from_deck_parts', ignore_case=True)),
        #     map(include('from_csv'), key=regex('from_csvs', ignore_case=True)),
        #     map(include('from_crowd_anki'), key=regex('from_crowd_anki', ignore_case=True))
        # )

        task_list = [f"map(include({val}), key=regex({val}, ignore_case=True))" for val in cls.known_task_dict().keys()]

        final_result: str = "list(\n\r" + ",\n\r".join(task_list) + "\n)\n\n"  # Add in a --- to here?
        final_extras: set = set()

        for val, extras in cls.known_validators():
            final_result = final_result + dedent(val) + "\n"
            if extras is not None:
                final_extras = final_extras.union(extras)

        return final_result, final_extras
