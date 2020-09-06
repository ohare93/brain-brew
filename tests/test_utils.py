import pytest

from brain_brew.utils import find_media_in_field, str_to_lowercase_no_separators, split_tags, join_tags


class TestFindMedia:
    @pytest.mark.parametrize("field_value, expected_results", [
        (r'<img src="image.png">', ["image.png"]),
        (r'<img src="image.png" >', ["image.png"]),
        (r'< img src="image.png">', ["image.png"]),
        (r'<     img src="image.png">', ["image.png"]),
        (r'<img class="myamainzingclass" src="image.png" >', ["image.png"]),
        (r'<img class=noquotesclass src="image.png" >', ["image.png"]),
        (r'<img src="image.png" class=classattheendforsomereason>', ["image.png"]),
        (r'words in the field <img src="image.png" > end other stuff', ["image.png"]),
        (r'<img src="ug-map-saint_barthelemy.png" />', ["ug-map-saint_barthelemy.png"]),
        (r'<img src="ug-map-saint_barthelemy.png" /><img src="image.png">',
            ["ug-map-saint_barthelemy.png", "image.png"]),
        (r'[sound:test.mp3]', ["test.mp3"]),
        (r'[sound:test.mp3][sound:othersound.mp3]', ["test.mp3", "othersound.mp3"]),
        (r'[sound:test.mp3]                       [sound:othersound.mp3]', ["test.mp3", "othersound.mp3"]),
        (r'words in the field [sound:test.mp3] other stuff too [sound:othersound.mp3] end', ["test.mp3", "othersound.mp3"]),
    ])
    def test_find_media_in_field(self, field_value, expected_results):
        assert find_media_in_field(field_value) == expected_results


class TestHelperFunctions:
    @pytest.mark.parametrize("str_to_tidy", [
        'Generate Csv Blah Blah',
        'Generate__Csv_Blah-Blah',
        'Generate      Csv Blah Blah',
        'generateCsvBlahBlah'
    ])
    def test_remove_spacers_from_str(self, str_to_tidy):
        assert str_to_lowercase_no_separators(str_to_tidy) == "generatecsvblahblah"


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
        assert split_tags(str_to_split) == expected_result


# class TestJoinTags:
#     @pytest.mark.parametrize("join_with, expected_result", [
#         (", ", "test, test1, test2")
#     ])
#     def test_joins(self, global_config, join_with, expected_result):
#         list_to_join = ["test", "test1", "test2"]
#         global_config.flags.join_values_with = join_with
#
#         assert join_tags(list_to_join) == expected_result
