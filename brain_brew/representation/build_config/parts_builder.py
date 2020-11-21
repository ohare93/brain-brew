from abc import ABCMeta
from dataclasses import dataclass
from textwrap import dedent
from typing import Dict, Type, List, Tuple

# Build Tasks
from brain_brew.build_tasks.csvs.notes_from_csvs import NotesFromCsvs  # noqa
from brain_brew.build_tasks.deck_parts.from_yaml_part import NotesFromYamlPart, HeadersFromYamlPart, NoteModelsFromYamlPart, MediaGroupFromYamlPart  # noqa
from brain_brew.build_tasks.deck_parts.media_group_from_folder import MediaGroupFromFolder  # noqa
from brain_brew.build_tasks.crowd_anki.headers_from_crowdanki import HeadersFromCrowdAnki  # noqa
from brain_brew.build_tasks.crowd_anki.media_group_from_crowd_anki import MediaGroupFromCrowdAnki  # noqa
from brain_brew.build_tasks.crowd_anki.note_models_from_crowd_anki import NoteModelsFromCrowdAnki  # noqa
from brain_brew.build_tasks.crowd_anki.notes_from_crowd_anki import NotesFromCrowdAnki  # noqa


from brain_brew.representation.build_config.build_task import BuildTask, BuildPartTask, TopLevelBuildTask
from brain_brew.representation.build_config.recipe_builder import RecipeBuilder


@dataclass
class PartsBuilder(RecipeBuilder, TopLevelBuildTask, metaclass=ABCMeta):
    @classmethod
    def task_regex(cls) -> str:
        return r'build_parts'

    @classmethod
    def known_task_dict(cls) -> Dict[str, Type[BuildTask]]:
        return BuildPartTask.get_all_task_regex()

    @classmethod
    def known_validators(cls) -> List[Tuple[str, set]]:
        return BuildPartTask.get_all_validators()

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

        task_list = [f"map(include('{val}'), key=regex('{val}', ignore_case=True))" for val in cls.known_task_dict().keys()]

        final_result: str = "list(\n\r" + ",\n\r".join(task_list) + "\n)\n\n---\n\n"
        final_extras: set = set()

        for val, extras in cls.known_validators():
            final_result = final_result + dedent(val) + "\n"
            if extras is not None:
                final_extras = final_extras.union(extras)

        return final_result, final_extras
