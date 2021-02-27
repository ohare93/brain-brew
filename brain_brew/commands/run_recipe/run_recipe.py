from dataclasses import dataclass
from brain_brew.interfaces.command import Command
from brain_brew.commands.run_recipe.top_level_builder import TopLevelBuilder
from brain_brew.configuration.file_manager import FileManager
from brain_brew.configuration.global_config import GlobalConfig
from brain_brew.configuration.yaml_verifier import YamlVerifier


@dataclass
class RunRecipe(Command):
    recipe_file_name: str
    global_config_file: str
    verify_only: bool

    def execute(self):
        # Read in Global Config File
        GlobalConfig.from_yaml_file(self.global_config_file) if self.global_config_file else GlobalConfig.from_yaml_file()
        FileManager()

        # Parse Build Config File
        YamlVerifier()
        recipe = TopLevelBuilder.parse_and_read(self.recipe_file_name, self.verify_only)

        if not self.verify_only:
            recipe.execute()
