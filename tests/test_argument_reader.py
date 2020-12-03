from argparse import ArgumentParser
from unittest.mock import patch

import pytest

from brain_brew.configuration.argument_reader import BBArgumentReader


@pytest.fixture()
def arg_reader_test1():
    return BBArgumentReader()


def test_constructor(arg_reader_test1):
    assert isinstance(arg_reader_test1, BBArgumentReader)
    assert isinstance(arg_reader_test1, ArgumentParser)


class TestArguments:
    @pytest.mark.parametrize("arguments", [
        (["test_recipe.yaml", "--config"]),
        (["test_recipe.yaml", "config_file.yaml", "--config"]),
        (["--config", "config_file.yaml"]),
        ([""]),
        ([])
    ])
    def test_broken_arguments(self, arg_reader_test1, arguments):
        def raise_exit(message):
            raise SystemExit

        with pytest.raises(SystemExit):
            with patch.object(BBArgumentReader, "error", side_effect=raise_exit):
                parsed_args = arg_reader_test1.get_parsed(arguments)

    @pytest.mark.parametrize("arguments, recipe, config_file, verify_only", [
        (["test_recipe.yaml"], "test_recipe.yaml", None, False),
        (["test_recipe.yaml", "--verify"], "test_recipe.yaml", None, True),
        (["test_recipe.yaml", "-v"], "test_recipe.yaml", None, True),
        (["test_recipe.yaml", "--config", "other_config.yaml"], "test_recipe.yaml", "other_config.yaml", False),
        (["test_recipe.yaml", "--config", "other_config.yaml", "-v"], "test_recipe.yaml", "other_config.yaml", True),
    ])
    def test_correct_arguments(self, arg_reader_test1, arguments, recipe, config_file, verify_only):
        parsed_args = arg_reader_test1.parse_args(arguments)

        assert parsed_args.recipe == recipe
        assert parsed_args.config_file == config_file
        assert parsed_args.verify_only == verify_only
