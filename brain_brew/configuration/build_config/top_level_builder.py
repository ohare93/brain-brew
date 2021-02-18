from textwrap import indent, dedent
from typing import Dict, Type, List, Set

from brain_brew.build_tasks.crowd_anki.crowd_anki_generate import CrowdAnkiGenerate
from brain_brew.build_tasks.csvs.csvs_generate import CsvsGenerate
from brain_brew.build_tasks.csvs.generate_guids_in_csvs import GenerateGuidsInCsvs
from brain_brew.configuration.build_config.build_task import BuildTask, TopLevelBuildTask
from brain_brew.configuration.build_config.parts_builder import PartsBuilder
from brain_brew.configuration.build_config.recipe_builder import RecipeBuilder
from brain_brew.interfaces.yamale_verifyable import YamlRepr


class TopLevelBuilder(YamlRepr, RecipeBuilder):
    @classmethod
    def known_task_dict(cls) -> Dict[str, Type[BuildTask]]:
        values = TopLevelBuildTask.get_all_task_regex(cls.yamale_dependencies())
        return values

    @classmethod
    def build_yamale(cls):
        separator = '\n---\n'
        top_level = cls.yamale_dependencies()

        builder: List[str] = [cls.build_yamale_root_node(top_level), separator]

        def to_sorted_yamale_string(lines: Set[Type[BuildTask]]):
            return [f'''{line.task_name()}:\n{indent(dedent(line.yamale_schema()), '    ')}'''
                    for line in sorted(lines, key=lambda x: x.task_name())]

        # Schema
        builder += to_sorted_yamale_string(top_level)

        builder.append(separator)

        # Dependencies
        def resolve_dependencies(deps: Set[Type[BuildTask]]) -> Set[Type[BuildTask]]:
            result = set()
            for d in deps:
                result.add(d)
                result = result.union(resolve_dependencies(d.yamale_dependencies()))
            return result

        children = resolve_dependencies(cls.yamale_dependencies())
        builder += to_sorted_yamale_string({dep for dep in children if dep not in top_level})

        return '\n'.join(builder)

    @classmethod
    def parse_and_read(cls, filename, verify_only: bool) -> 'TopLevelBuilder':
        recipe_data = cls.read_to_dict(filename)

        from brain_brew.configuration.yaml_verifier import YamlVerifier
        YamlVerifier.get_instance().verify_recipe(filename)

        if verify_only:
            return None

        return cls.from_list(recipe_data)

    @classmethod
    def task_name(cls) -> str:
        pass

    @classmethod
    def yamale_schema(cls) -> str:
        pass

    @classmethod
    def from_repr(cls, data: dict):
        pass

    def encode(self) -> dict:
        pass

    @classmethod
    def from_yaml_file(cls, filename: str):
        pass

    @classmethod
    def yamale_dependencies(cls) -> Set[Type[TopLevelBuildTask]]:
        return {CrowdAnkiGenerate, CsvsGenerate, PartsBuilder, GenerateGuidsInCsvs}
