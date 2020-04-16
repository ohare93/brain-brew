import os
import pathlib
import string
import random
import re


def blank_str_if_none(s):
    return '' if s is None else s


def list_of_str_to_lowercase(list_of_strings):
    return [entry.lower() for entry in list_of_strings]


def single_item_to_list(item):
    if isinstance(item, list):
        return item
    return [item]


def filename_from_full_path(full_path):
    return re.findall('[^\\/:*?"<>|\r\n]+$', full_path)[0]


def find_all_files_in_directory(directory, recursive=False):
    found_files = []
    for path, dirs, files in os.walk(directory):
        for file in files:
            found_files.append(os.path.join(path, file))
        if not recursive:
            return found_files
    return found_files


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
