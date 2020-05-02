from argparse import ArgumentParser

import pytest

from brain_brew.argument_reader import BBArgumentReader


@pytest.fixture()
def arg_reader_test1():
    return BBArgumentReader()


def test_constructor(arg_reader_test1):
    assert isinstance(arg_reader_test1, BBArgumentReader)
    assert isinstance(arg_reader_test1, ArgumentParser)


class TestArguments:
    @pytest.mark.parametrize("arguments", [
        (["test_builder.yaml", "--config"]),
        (["test_builder.yaml", "config_file.yaml", "--config"]),
        (["--config", "config_file.yaml"]),
        ([""]),
        ([])
    ])
    def test_broken_arguments(self, arg_reader_test1, arguments):
        with pytest.raises(SystemExit):
            parsed_args = arg_reader_test1.get_parsed(arguments)

    @pytest.mark.parametrize("arguments, builder_file, config_file", [
        (["test_builder.yaml"], "test_builder.yaml", None),
        (["test_builder.yaml", "--config", "other_config.yaml"], "test_builder.yaml", "other_config.yaml"),
    ])
    def test_correct_arguments(self, arg_reader_test1, arguments, builder_file, config_file):
        parsed_args = arg_reader_test1.parse_args(arguments)

        assert parsed_args.builder_file == builder_file
        assert parsed_args.config_file == config_file
