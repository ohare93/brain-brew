from dataclasses import dataclass
from brain_brew.interfaces.command import Command
from brain_brew.commands.run_recipe.top_level_builder import TopLevelBuilder
from brain_brew.configuration.yaml_verifier import YamlVerifier


@dataclass
class RunRecipe(Command):
    recipe_file_name: str
    verify_only: bool

    def execute(self):
        # Parse Build Config File
        YamlVerifier()
        recipe = TopLevelBuilder.parse_and_read(self.recipe_file_name, self.verify_only)

        if not self.verify_only:
            recipe.execute()
