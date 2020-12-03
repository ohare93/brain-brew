import sys
from argparse import ArgumentParser


class BBArgumentReader(ArgumentParser):
    def __init__(self):
        super().__init__(
            description='Manage Flashcards by Transforming them to various types'
        )

        self._set_parser_arguments()

    def _set_parser_arguments(self):
        self.add_argument(
            "recipe",
            metavar="recipe",
            type=str,
            help="Yaml file to use as the recipe"
        )
        self.add_argument(
            "--config", "--global-config", "-c",
            action="store",
            dest="config_file",
            default=None,
            type=str,
            help="Global config file to use"
        )
        self.add_argument(
            "--verify", "-v",
            action="store_true",
            dest="verify_only",
            default=False,
            help="Only verify the recipe contents, without running it."
        )

    def get_parsed(self, override_args=None):
        parsed_args = self.parse_args(args=override_args)

        # Required
        recipe = self.error_if_blank(parsed_args.recipe)

        # Optional
        config_file = parsed_args.config_file
        verify_only = parsed_args.verify_only

        return recipe, config_file, verify_only

    def error_if_blank(self, arg):
        if arg == "" or arg is None:
            self.error("Required argument missing")
        return arg

    def error(self, message):
        sys.stderr.write('error: %s\n' % message)
        self.print_help()
        sys.exit(2)
