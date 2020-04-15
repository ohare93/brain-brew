import pytest

from brain_brew.build_tasks.build_task_generic import BuildTaskGeneric
from tests.test_helpers import global_config


class TestSplitTags:
    @pytest.mark.parametrize("str_to_split, expected_result", [
        ("tags1, tags2", ["tags1", "tags2"]),
        ("tags1 tags2", ["tags1", "tags2"]),
        ("tags1; tags2", ["tags1", "tags2"]),
        ("tags1      tags2", ["tags1", "tags2"]),
        ("tags1, tags2, tags3, tags4, tags5, tags6, tags7, tags8, tags9",
            ["tags1", "tags2", "tags3", "tags4", "tags5", "tags6", "tags7", "tags8", "tags9"]),
        ("tags1, tags2; tags3 tags4      tags5,     tags6;    tags7    tags8, tags9",
         ["tags1", "tags2", "tags3", "tags4", "tags5", "tags6", "tags7", "tags8", "tags9"]),
        ("tags1,tags2", ["tags1", "tags2"]),
        ("tags1;tags2", ["tags1", "tags2"]),
        ("tags1,    tags2", ["tags1", "tags2"]),
        ("tags1;    tags2", ["tags1", "tags2"]),
    ])
    def test_runs(self, str_to_split, expected_result):
        assert BuildTaskGeneric.split_tags(str_to_split) == expected_result


class TestJoinTags:
    @pytest.mark.parametrize("join_with, expected_result", [
        (", ", "test, test1, test2")
    ])
    def test_joins(self, global_config, join_with, expected_result):
        list_to_join = ["test", "test1", "test2"]
        global_config.flags.join_values_with = join_with

        assert BuildTaskGeneric.join_tags(list_to_join) == expected_result
