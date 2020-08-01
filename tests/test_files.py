

class TestFiles:
    class Headers:
        LOC = "tests/test_files/deck_parts/headers/"

        FIRST = "default header"
        FIRST_COMPLETE = "default-header.json"

    class NoteFiles:
        LOC = "tests/test_files/deck_parts/"

        TEST1_NO_GROUPING_OR_SHARED_TAGS = "csvtonotes1_withnogroupingorsharedtags.json"
        TEST1_WITH_GROUPING = "csvtonotes1_withgrouping.json"
        TEST1_WITH_SHARED_TAGS = "csvtonotes1_withsharedtags.json"
        TEST1_WITH_SHARED_TAGS_EMPTY_AND_GROUPING = "csvtonotes1_withsharedtagsandgrouping_butnothingtogroup.json"
        TEST2_WITH_SHARED_TAGS_AND_GROUPING = "csvtonotes2_withsharedtagsandgrouping.json"

    class NoteModels:
        LOC = "tests/test_files/deck_parts/note_models/"

        TEST = "Test Model"
        TEST_COMPLETE = LOC + "Test-Model.json"

        LL_WORD = "LL Word"
        LL_WORD_COMPLETE = LOC + "LL-Word.json"

    class CsvFiles:
        LOC = "tests/test_files/csv/"

        TEST1 = LOC + "test1.csv"
        TEST1_SPLIT1 = LOC + "test1_split1.csv"
        TEST1_SPLIT2 = LOC + "test1_split2.csv"
        TEST2 = LOC + "test2.csv"
        TEST2_MISSING_GUIDS = LOC + "test2_missing_guids.csv"
        TEST3 = LOC + "test3.csv"

    class CrowdAnkiExport:
        LOC = "tests/test_files/crowd_anki/"

        TEST1_FOLDER = LOC + "crowdanki_example_1/"
        TEST1_FOLDER_WITHOUT_SLASH = LOC + "crowdanki_example_1"
        TEST1_JSON = TEST1_FOLDER + "deck.json"

    class BuildConfig:
        LOC = "tests/test_files/build_files/"

        ONE_OF_EACH_TYPE = LOC + "builder1.yaml"

    class MediaFiles:
        LOC = "tests/test_files/media_files/"

    class YamlNotes:
        LOC = "tests/test_files/yaml/note/"

        TEST1 = LOC + "note1.yaml"
