from enum import Enum
from typing import Tuple

import sys
from argparse import ArgumentParser

from brain_brew.commands.init_repo.init_repo import InitRepo
from brain_brew.commands.run_recipe.run_recipe import RunRecipe
from brain_brew.interfaces.command import Command


class Commands(Enum):
    RUN_RECIPE = "run"
    INIT_REPO = "init"


class BBArgumentReader(ArgumentParser):
    def __init__(self):
        super().__init__(
            prog="Brain Brew",
            description='Manage Flashcards by Transforming them to various types'
        )

        self._set_parser_arguments()

    def _set_parser_arguments(self):

        subparsers = self.add_subparsers(parser_class=ArgumentParser, help='Commands that can be run', dest="command")

        parser_run = subparsers.add_parser(
            Commands.RUN_RECIPE.value,
            help="Run a recipe file. This will convert some data to another format, based on the instructions in the recipe file."
        )
        parser_run.add_argument(
            "recipe",
            metavar="recipe",
            type=str,
            help="Yaml file to use as the recipe"
        )
        parser_run.add_argument(
            "--config", "--global-config", "-c",
            action="store",
            dest="config_file",
            default=None,
            type=str,
            help="Global config file to use"
        )
        parser_run.add_argument(
            "--verify", "-v",
            action="store_true",
            dest="verify_only",
            default=False,
            help="Only verify the recipe contents, without running it."
        )

        parser_init = subparsers.add_parser(
            Commands.INIT_REPO.value,
            help="Initialise a Brain Brew repository, using a CrowdAnki file for the base data."
        )
        parser_init.add_argument(
            "crowdanki_folder",
            metavar="crowdanki_folder",
            type=str,
            help="The folder that stores the CrowdAnki files to build this repo from"
        )

    def get_parsed(self, override_args=None) -> Command:
        parsed_args = self.parse_args(args=override_args)

        if parsed_args.command == Commands.RUN_RECIPE.value:
            # Required
            recipe = self.error_if_blank(parsed_args.recipe)

            # Optional
            config_file = parsed_args.config_file
            verify_only = parsed_args.verify_only

            return RunRecipe(
                recipe_file_name=recipe,
                global_config_file=config_file,
                verify_only=verify_only
            )

        if parsed_args.command == Commands.INIT_REPO.value:
            # Required
            crowdanki_folder = parsed_args.crowdanki_folder

            return InitRepo(
                crowdanki_folder=crowdanki_folder
            )

        raise KeyError("Unknown Command")

    def error_if_blank(self, arg):
        if arg == "" or arg is None:
            self.error("Required argument missing")
        return arg

    def error(self, message):
        sys.stderr.write('error: %s\n' % message)
        self.print_help()
        sys.exit(2)
