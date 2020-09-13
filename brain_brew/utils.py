import logging
import os
from pathlib import Path
import string
import random
import re
from typing import List


def blank_str_if_none(s):
    return '' if s is None else s


def list_of_str_to_lowercase(list_of_strings):
    return [entry.lower() for entry in list_of_strings]


def single_item_to_list(item):
    if isinstance(item, list):
        return item
    if item is None:
        return []
    return [item]


def all_combos_prepend_append(original_list: list, prepend_with: str, append_with: str):
    return list({append_or_not for normal in original_list
                 for prepend_or_not in (normal, prepend_with + normal)
                 for append_or_not in (prepend_or_not, prepend_or_not + append_with)})


def str_to_lowercase_no_separators(str_to_tidy: str):
    return re.sub(r'[\s+_-]+', '', str_to_tidy.lower())


def filename_from_full_path(full_path):
    return re.findall('[^\\/:*?"<>|\r\n]+$', full_path)[0]


def find_media_in_field(field_value: str) -> List[str]:
    if not field_value:
        return []

    images = re.findall(r'<\s*?img.*?src="(.*?)"[^>]*?>', field_value)
    audio = re.findall(r'\[sound:(.*?)\]', field_value)

    return images + audio


def find_all_files_in_directory(directory, recursive=False):
    found_files = []
    for path, dirs, files in os.walk(directory):
        for file in files:
            found_files.append(os.path.join(path, file))
        if not recursive:
            return found_files
    return found_files


def create_path_if_not_exists(path):
    dir_name = os.path.dirname(path)
    if not Path().is_dir():
        logging.warning(f"Creating missing filepath '{dir_name}'")
        os.makedirs(os.path.dirname(dir_name), exist_ok=True)


def split_tags(tags_value: str) -> list:
    split = [entry.strip() for entry in re.split(r';\s*|,\s*|\s+', tags_value)]
    while "" in split:
        split.remove("")
    return split


def join_tags(tags_list: list) -> str:
    from brain_brew.representation.configuration.global_config import GlobalConfig
    return GlobalConfig.get_instance().join_values_with.join(tags_list)


def generate_anki_guid() -> str:
    """Return a base91-encoded 64bit random number."""

    def base62(num: int, extra: str = "") -> str:
        s = string
        table = s.ascii_letters + s.digits + extra
        buf = ""
        while num:
            num, i = divmod(num, len(table))
            buf = table[i] + buf
        return buf

    _base91_extra_chars = "!#$%&()*+,-./:;<=>?@[]^_`{|}~"

    def base91(num: int) -> str:
        # all printable characters minus quotes, backslash and separators
        return base62(num, _base91_extra_chars)

    return base91(random.randint(0, 2 ** 64 - 1))


def sort_dict(data, sort_by_keys, reverse_sort, case_insensitive_sort=None):
    from brain_brew.representation.configuration.global_config import GlobalConfig
    if case_insensitive_sort is None:
        case_insensitive_sort = GlobalConfig.get_instance().sort_case_insensitive

    if sort_by_keys:
        if case_insensitive_sort:
            def sort_method(i):
                return tuple((i[column] == "", i[column].lower()) for column in sort_by_keys)
        else:
            def sort_method(i):
                return tuple((i[column] == "", i[column]) for column in sort_by_keys)

        return sorted(data, key=sort_method, reverse=reverse_sort)
    elif reverse_sort:
        return list(reversed(data))

    return data
