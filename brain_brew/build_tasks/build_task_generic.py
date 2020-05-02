import re

from brain_brew.representation.configuration.global_config import GlobalConfig


class BuildTaskGeneric:
    @staticmethod
    def split_tags(tags_value: str) -> list:
        split = [entry.strip() for entry in re.split(';\s*|,\s*|\s+', tags_value)]
        while "" in split:
            split.remove("")
        return split

    @staticmethod
    def join_tags(tags_list: list) -> str:
        return GlobalConfig.get_instance().flags.join_values_with.join(tags_list)
