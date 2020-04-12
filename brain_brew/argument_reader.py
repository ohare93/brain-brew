from argparse import ArgumentParser


class ArgumentReader:
    parser: ArgumentParser

    def __init__(self):
        self.parser = ArgumentParser(
            description='Manage Flashcards by Transforming them to various types'
        )

        self._set_parser_arguments()

    def _set_parser_arguments(self):
        self.parser.add_argument(
            "builder",
            metavar="file",
            type=str,
            help="Run a builder file"
        )

    def get_parsed(self):
        parsed_args = self.parser.parse_args()

        builder = parsed_args.builder
        other_arguments = []  # TODO: Add other arguments to parser

        return builder, other_arguments
