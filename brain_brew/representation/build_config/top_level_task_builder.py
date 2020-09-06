import logging
from typing import Dict, Type

from brain_brew.representation.build_config.build_task import BuildTask, TopLevelBuildTask
from brain_brew.representation.build_config.task_builder import TaskBuilder

# Build Tasks
from brain_brew.build_tasks.csvs.csvs_generate import CsvsGenerate
from brain_brew.build_tasks.crowd_anki.crowd_anki_generate import CrowdAnkiGenerate
from brain_brew.representation.build_config.generate_deck_parts import GenerateDeckParts
from brain_brew.utils import str_to_lowercase_no_separators


class TopLevelTaskBuilder(TaskBuilder):
    @classmethod
    def known_task_dict(cls) -> Dict[str, Type[BuildTask]]:
        values = TopLevelBuildTask.get_all_task_regex()
        return values

    @classmethod
    def parse_and_read(cls, filename) -> 'TopLevelTaskBuilder':
        builder_data = TopLevelTaskBuilder.read_to_dict(filename)

        from brain_brew.yaml_verifier import YamlVerifier
        YamlVerifier.get_instance().verify_builder(filename)

        return TopLevelTaskBuilder.from_list(builder_data)
