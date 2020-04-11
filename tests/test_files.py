

class TestFiles:
    class CsvFiles:
        LOC = "tests/test_files/csv/"

        TEST1 = LOC + "test1.csv"
        TEST2 = LOC + "test2.csv"
        TEST1_SPLIT1 = LOC + "test1_split1.csv"
        TEST1_SPLIT2 = LOC + "test1_split2.csv"

    class UnfinishedData:
        LOC = "tests/test_files/unfinished_data/"

        FIRST_SET = LOC + "test1_after_csv_mapping.json"
        SECOND_SET = LOC + "test2_after_csv_mapping.json"
        FIRST_SET_SPLIT1 = LOC + "test3_after_csv_mapping.json"
        FIRST_SET_SPLIT2 = LOC + "test4_after_csv_mapping.json"

    class Headers:
        LOC = "tests/test_files/deck_parts/headers/"

        FIRST = LOC + "default header"
        FIRST_FULL = LOC + "default-header.json"

    class NoteFiles:
        LOC = "tests/test_files/deck_parts/"

        NO_GROUPING_OR_SHARED_TAGS = LOC + "csvtonotes1_withnogroupingorsharedtags.json"
        WITH_GROUPING = LOC + "csvtonotes1_withgrouping.json"
        WITH_SHARED_TAGS = LOC + "csvtonotes1_withsharedtags.json"
        WITH_SHARED_TAGS_EMPTY_AND_GROUPING = LOC + "csvtonotes1_withsharedtagsandgrouping_butnothingtogroup.json"
        WITH_SHARED_TAGS_AND_GROUPING = LOC + "csvtonotes2_withsharedtagsandgrouping.json"

    class CrowdAnkiExport:
        LOC = "tests/test_files/crowd_anki/"

        TEST1_FOLDER = LOC + "crowdanki_example_1/"
        TEST1_FOLDER_WITHOUT_SLASH = LOC + "crowdanki_example_1"
        TEST1_JSON = TEST1_FOLDER + "deck.json"

    class NoteModels:
        LOC = "tests/test_files/deck_parts/note_models/"

        LL_NOUN = LOC + "Dummy LL Noun"
        LL_NOUN_FULL = LOC + "Dummy-LL-Noun.json"

        LL_WORD = LOC + "LL Word"
        LL_WORD_FULL = LOC + "LL_Word.json"

    class BuildConfig:
        LOC = "tests/test_files/build_files/"

        ONE_OF_EACH_TYPE = LOC + "builder1.yaml"
