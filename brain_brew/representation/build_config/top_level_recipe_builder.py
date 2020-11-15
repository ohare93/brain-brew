import logging
from abc import ABC
from typing import Dict, Type

from brain_brew.representation.build_config.build_task import BuildTask, TopLevelBuildTask
from brain_brew.representation.build_config.recipe_builder import RecipeBuilder

# Build Tasks
from brain_brew.build_tasks.csvs.csvs_generate import CsvsGenerate  # noqa
from brain_brew.build_tasks.crowd_anki.crowd_anki_generate import CrowdAnkiGenerate  # noqa
from brain_brew.representation.build_config.generate_deck_parts import BuildDeckParts  # noqa


class TopLevelRecipeBuilder(RecipeBuilder, ABC):
    @classmethod
    def known_task_dict(cls) -> Dict[str, Type[BuildTask]]:
        values = TopLevelBuildTask.get_all_task_regex()
        return values

    @classmethod
    def parse_and_read(cls, filename) -> 'TopLevelRecipeBuilder':
        recipe_data = TopLevelRecipeBuilder.read_to_dict(filename)

        from brain_brew.yaml_verifier import YamlVerifier
        YamlVerifier.get_instance().verify_recipe(filename)

        return TopLevelRecipeBuilder.from_list(recipe_data)
