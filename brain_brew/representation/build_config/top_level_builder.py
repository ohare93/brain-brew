from abc import ABCMeta
from typing import Dict, Type, List, Tuple, Set
from textwrap import indent, dedent

# Build Tasks
from brain_brew.build_tasks.crowd_anki.crowd_anki_generate import CrowdAnkiGenerate
from brain_brew.build_tasks.csvs.csvs_generate import CsvsGenerate
from brain_brew.representation.build_config.parts_builder import PartsBuilder

from brain_brew.interfaces.yamale_verifyable import YamlRepr
from brain_brew.representation.build_config.recipe_builder import RecipeBuilder
from brain_brew.representation.build_config.build_task import BuildTask, TopLevelBuildTask


class TopLevelBuilder(YamlRepr, RecipeBuilder, metaclass=ABCMeta):
    @classmethod
    def known_task_dict(cls) -> Dict[str, Type[BuildTask]]:
        values = TopLevelBuildTask.get_all_task_regex(cls.yamale_dependencies())
        return values

    @classmethod
    def build_yamale(cls):
        separator = '\n---\n'
        top_level = cls.yamale_dependencies()

        builder: List[str] = [cls.build_yamale_root_node(top_level), separator]

        def to_yamale_string(c: Type[BuildTask]):
            return f'''{c.task_name()}:\n{indent(dedent(c.yamale_schema()), '    ')}'''

        # Schema
        for dep in top_level:
            builder.append(to_yamale_string(dep))

        builder.append(separator)

        # Dependencies
        def resolve_dependencies(deps: Set[Type[BuildTask]]) -> Set[Type[BuildTask]]:
            result = set()
            for d in deps:
                result.add(d)
                result = result.union(resolve_dependencies(d.yamale_dependencies()))
            return result

        children = resolve_dependencies(cls.yamale_dependencies())
        for dep in children:
            if dep not in top_level:
                builder.append(to_yamale_string(dep))

        return '\n'.join(builder)

    @classmethod
    def parse_and_read(cls, filename) -> 'TopLevelBuilder':
        recipe_data = cls.read_to_dict(filename)

        from brain_brew.yaml_verifier import YamlVerifier
        YamlVerifier.get_instance().verify_recipe(filename)

        return cls.from_list(recipe_data)

    @classmethod
    def yamale_dependencies(cls) -> Set[Type[TopLevelBuildTask]]:
        return {CrowdAnkiGenerate, CsvsGenerate, PartsBuilder}
