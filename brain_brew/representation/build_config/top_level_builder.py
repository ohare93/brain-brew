from abc import ABCMeta
from typing import Dict, Type

# Build Tasks
from brain_brew.build_tasks.crowd_anki.crowd_anki_generate import CrowdAnkiGenerate  # noqa
from brain_brew.build_tasks.csvs.csvs_generate import CsvsGenerate  # noqa
from brain_brew.representation.build_config.parts_builder import PartsBuilder  # noqa

from brain_brew.representation.build_config.recipe_builder import RecipeBuilder
from brain_brew.representation.build_config.build_task import BuildTask, TopLevelBuildTask


class TopLevelBuilder(RecipeBuilder, metaclass=ABCMeta):
    @classmethod
    def known_task_dict(cls) -> Dict[str, Type[BuildTask]]:
        values = TopLevelBuildTask.get_all_task_regex()
        return values

    @classmethod
    def parse_and_read(cls, filename) -> 'TopLevelBuilder':
        recipe_data = TopLevelBuilder.read_to_dict(filename)

        from brain_brew.yaml_verifier import YamlVerifier
        YamlVerifier.get_instance().verify_recipe(filename)

        return TopLevelBuilder.from_list(recipe_data)
