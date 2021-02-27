from argparse import ArgumentParser, ArgumentError
from unittest.mock import patch

import pytest

from brain_brew.commands.argument_reader import BBArgumentReader, Commands


@pytest.fixture()
def arg_reader_test1():
    return BBArgumentReader()


def test_constructor(arg_reader_test1):
    assert isinstance(arg_reader_test1, BBArgumentReader)
    assert isinstance(arg_reader_test1, ArgumentParser)


class TestArguments:
    class CommandRun:
        @pytest.mark.parametrize("arguments", [
            ([Commands.RUN_RECIPE.value]),
            ([Commands.RUN_RECIPE.value, ""]),
        ])
        def test_broken_arguments(self, arg_reader_test1, arguments):
            def raise_exit(message):
                raise SystemExit

            with pytest.raises(SystemExit):
                with patch.object(BBArgumentReader, "error", side_effect=raise_exit):
                    arg_reader_test1.get_parsed(arguments)

        @pytest.mark.parametrize("arguments, recipe, verify_only", [
            ([Commands.RUN_RECIPE.value, "test_recipe.yaml"], "test_recipe.yaml", False),
            ([Commands.RUN_RECIPE.value, "test_recipe.yaml", "--verify"], "test_recipe.yaml", True),
            ([Commands.RUN_RECIPE.value, "test_recipe.yaml", "-v"], "test_recipe.yaml", True),
        ])
        def test_correct_arguments(self, arg_reader_test1, arguments, recipe, verify_only):
            parsed_args = arg_reader_test1.parse_args(arguments)

            assert parsed_args.recipe == recipe
            assert parsed_args.verify_only == verify_only

    class CommandInit:
        @pytest.mark.parametrize("arguments, location", [
            (["init", "crowdankifolder72"], "crowdankifolder72"),
        ])
        def test_correct_arguments(self, arg_reader_test1, arguments, location):
            parsed_args = arg_reader_test1.parse_args(arguments)

            assert parsed_args.crowdanki_folder == location
