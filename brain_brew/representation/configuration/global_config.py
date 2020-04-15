from dataclasses import dataclass, field

from brain_brew.constants.deckpart_keys import NoteFlagKeys, DeckPartNoteFlags
from brain_brew.constants.global_config_keys import *
from brain_brew.representation.configuration.yaml_file import YamlFile, ConfigKey


@dataclass
class DeckPartConfig:
    headers: str
    note_models: str
    notes: str
    media_files: str


class GlobalConfig(YamlFile):
    __instance = None

    @dataclass
    class ConfigFlags:
        note_sort_order: list = field(default_factory=list)
        sort_case_insensitive: bool = False
        reverse_sort: bool = False
        join_values_with: str = " "

    authors: object
    deck_parts: DeckPartConfig
    flags: ConfigFlags
    deck_part_notes_flags: DeckPartNoteFlags
    join_values_with: str

    config_entry = {}
    expected_keys = {
        ConfigKeys.AUTHORS.value: ConfigKey(False, list, None),
        ConfigKeys.DECK_PARTS.value: ConfigKey(True, dict, {
            ConfigKeys.HEADERS.value: ConfigKey(True, str, None),
            ConfigKeys.NOTE_MODELS.value: ConfigKey(True, str, None),
            ConfigKeys.NOTES.value: ConfigKey(True, str, None),
            ConfigKeys.MEDIA_FILES.value: ConfigKey(True, str, None),

            ConfigKeys.DECK_PARTS_NOTES_STRUCTURE.value: ConfigKey(True, dict, {
                flag.value: ConfigKey(False, bool, None) for flag in NoteFlagKeys
            })
        }),
        ConfigKeys.FLAGS.value: ConfigKey(False, dict, {
            ConfigKeys.NOTE_SORT_ORDER.value: ConfigKey(False, list, None),
            ConfigKeys.SORT_CASE_INSENSITIVE.value: ConfigKey(False, bool, None),
            ConfigKeys.REVERSE_SORT.value: ConfigKey(False, bool, None),
            ConfigKeys.JOIN_VALUES_WITH.value: ConfigKey(False, str, None),
        })
    }
    subconfig_filter = None

    def __init__(self, config_data: dict):
        if GlobalConfig.__instance is None:
            GlobalConfig.__instance = self
        else:
            raise Exception("Multiple GlobalConfigs created")

        self.setup_config_with_subconfig_replacement(config_data)
        self.verify_config_entry()

        self.authors = self.get_config(ConfigKeys.AUTHORS, {})
        dp = self.get_config(ConfigKeys.DECK_PARTS)
        self.deck_parts = DeckPartConfig(
            dp[ConfigKeys.HEADERS.value], dp[ConfigKeys.NOTE_MODELS.value],
            dp[ConfigKeys.NOTES.value], dp[ConfigKeys.MEDIA_FILES.value]
        )

        self.flags = GlobalConfig.ConfigFlags()
        flag_keys = self.get_config(ConfigKeys.FLAGS, {})
        for flag in self.expected_keys[ConfigKeys.FLAGS.value].children.keys():
            if flag in flag_keys:
                setattr(self.flags, flag, flag_keys[flag])

        self.deck_part_notes_flags = DeckPartNoteFlags()
        note_flags = dp[ConfigKeys.DECK_PARTS_NOTES_STRUCTURE.value]
        for flag in note_flags.keys():
            setattr(self.deck_part_notes_flags, flag, note_flags[flag])

    @classmethod
    def get_default(cls):
        global_config_name = "_config.yaml"
        global_config_filepath = f"{global_config_name}"

        gb_config_yaml = YamlFile.read_file(global_config_filepath)

        return GlobalConfig(gb_config_yaml)

    @classmethod
    def get_instance(cls):
        return cls.__instance

    @classmethod
    def clear_instance(cls):
        if cls.__instance:
            cls.__instance = None
