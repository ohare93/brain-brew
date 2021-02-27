from dataclasses import dataclass
from typing import Dict, Type, List, Set

from brain_brew.build_tasks.crowd_anki.headers_from_crowdanki import HeadersFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.media_group_from_crowd_anki import MediaGroupFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.note_models_from_crowd_anki import NoteModelsFromCrowdAnki
from brain_brew.build_tasks.crowd_anki.notes_from_crowd_anki import NotesFromCrowdAnki
from brain_brew.build_tasks.csvs.notes_from_csvs import NotesFromCsvs
from brain_brew.build_tasks.deck_parts.from_yaml_part import NotesFromYamlPart, NoteModelsFromYamlPart, \
    MediaGroupFromYamlPart
from brain_brew.build_tasks.deck_parts.headers_from_yaml_part import HeadersFromYamlPart
from brain_brew.build_tasks.deck_parts.media_group_from_folder import MediaGroupFromFolder
from brain_brew.build_tasks.deck_parts.media_group_to_folder import MediaGroupsToFolder
from brain_brew.build_tasks.deck_parts.note_model_from_html_parts import NoteModelFromHTMLParts
from brain_brew.build_tasks.deck_parts.note_model_template_from_html_files import TemplateFromHTML
from brain_brew.commands.run_recipe.build_task import BuildTask, BuildPartTask, TopLevelBuildTask
from brain_brew.commands.run_recipe.recipe_builder import RecipeBuilder


@dataclass
class PartsBuilder(RecipeBuilder, TopLevelBuildTask):
    accepts_list_of_self: bool = False

    @classmethod
    def task_name(cls) -> str:
        return r'build_parts'

    @classmethod
    def task_regex(cls) -> str:
        return r'build_part[s]?'

    @classmethod
    def known_task_dict(cls) -> Dict[str, Type[BuildTask]]:
        return BuildPartTask.get_all_task_regex(cls.yamale_dependencies())

    @classmethod
    def from_repr(cls, data: List[dict]):
        if not isinstance(data, list):
            raise TypeError(f"PartsBuilder needs a list")
        return cls.from_list(data)

    def encode(self) -> dict:
        pass

    @classmethod
    def from_yaml_file(cls, filename: str):
        pass

    @classmethod
    def yamale_schema(cls) -> str:
        return cls.build_yamale_root_node(cls.yamale_dependencies())

    @classmethod
    def yamale_dependencies(cls) -> Set[Type[BuildPartTask]]:
        return {
            NotesFromCsvs,
            NotesFromYamlPart, HeadersFromYamlPart, NoteModelsFromYamlPart, MediaGroupFromYamlPart,
            MediaGroupFromFolder, MediaGroupsToFolder,
            NoteModelFromHTMLParts, TemplateFromHTML,
            HeadersFromCrowdAnki, MediaGroupFromCrowdAnki, NoteModelsFromCrowdAnki, NotesFromCrowdAnki
        }
