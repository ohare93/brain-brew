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
            "builder_file",
            metavar="builder file",
            type=str,
            help="Run a builder file"
        )
        self.add_argument(
            "--config", "--global-config", "-c",
            action="store",
            dest="config_file",
            default=None,
            type=str,
            help="Global config file to use"
        )

    def get_parsed(self, override_args=None):
        parsed_args = self.parse_args(args=override_args)

        # Required
        builder = self.error_if_blank(parsed_args.builder_file)

        # Optional
        config_file = parsed_args.config_file

        return builder, config_file

    def error_if_blank(self, arg):
        if arg == "" or arg is None:
            self.error("Required argument missing")
        return arg

    def error(self, message):
        sys.stderr.write('error: %s\n' % message)
        self.print_help()
        sys.exit(2)
