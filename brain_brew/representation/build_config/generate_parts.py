from abc import ABC
from dataclasses import dataclass
from typing import Dict, Type, List, Tuple
from textwrap import dedent

from brain_brew.representation.build_config.build_task import BuildTask, TopLevelBuildTask, PartBuildTask
from brain_brew.representation.build_config.recipe_builder import RecipeBuilder

# Build Tasks
from brain_brew.build_tasks.parts.from_yaml_part import NotesFromYamlPart, HeadersFromYamlPart, NoteModelsFromYamlPart, MediaGroupFromYamlPart  # noqa
from brain_brew.build_tasks.csvs.notes_from_csvs import NotesFromCsvs  # noqa
from brain_brew.build_tasks.parts.media_group_from_folder import MediaGroupFromSource, MediaGroupFromFolder  # noqa


@dataclass
class BuildParts(RecipeBuilder, TopLevelBuildTask, ABC):
    @classmethod
    def task_regex(cls) -> str:
        return r'build_parts'

    @classmethod
    def known_task_dict(cls) -> Dict[str, Type[BuildTask]]:
        return PartBuildTask.get_all_task_regex()

    @classmethod
    def known_validators(cls) -> List[Tuple[str, set]]:
        return PartBuildTask.get_all_validators()

    @classmethod
    def from_repr(cls, data: List[dict]):
        if not isinstance(data, list):
            raise TypeError(f"GenerateDeckParts needs a list")
        return cls.from_list(data)

    @classmethod
    def yamale_validator_and_deps(cls) -> (str, set):
        # list(
        #     map(include('from_parts'), key=regex('from_parts', ignore_case=True)),
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
