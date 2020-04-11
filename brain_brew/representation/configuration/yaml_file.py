from collections import namedtuple
from enum import Enum
from pathlib import Path
from typing import List, Dict

import yaml

from brain_brew.build_tasks.build_task_generic import BuildConfigKeys


ConfigKey = namedtuple("configkey", "required entry_type children")


class YamlFile:
    config_entry: dict
    expected_keys: dict
    subconfig_filter: list

    @staticmethod
    def read_file(src: str):
        if src[-5:] not in [".yaml", ".yml"]:
            src += ".yaml"

        if not Path(src).is_file():
            raise FileNotFoundError(src)

        with open(src, 'r') as yml_file:
            f = yaml.full_load(yml_file)

        return f

    @staticmethod
    def check_config_recursive(expected_keys, keys, parent_key_name=""):
        errors = []

        keys_left_to_check = list(expected_keys.keys())

        for key in keys:
            key_name = f"{parent_key_name}/{key}"

            if key in keys_left_to_check:
                keys_left_to_check.remove(key)

                if expected_keys[key].children:
                    errors = errors + \
                             YamlFile.check_config_recursive(expected_keys[key].children, keys[key], key_name)

                if not isinstance(keys[key], expected_keys[key].entry_type):
                    errors.append(f"Expected '{key_name}' to be of type {expected_keys[key].entry_type}"
                                  f", not {type(keys[key])}")

            else:
                errors.append(f"Unexpected key '{key_name}'")

        for key in keys_left_to_check:
            if expected_keys[key].required:
                errors.append(f"Missing key '{key}'")

        return errors

    def verify_config_entry(self):
        errors = YamlFile.check_config_recursive(self.expected_keys, self.config_entry)
        if errors:
            self.raise_error_in_config(errors)

    def get_config(self, enum_key, otherwise=None):
        if enum_key.value in self.config_entry:
            return self.config_entry[enum_key.value]
        if otherwise is not None:
            return otherwise
        raise KeyError(f"Expected key {enum_key.value}")

    def raise_error_in_config(self, error_message):
        raise KeyError(error_message)

    def setup_config_with_subconfig_replacement(self, config_entry: dict):
        SUBCONFIG = BuildConfigKeys.SUBCONFIG.value

        if SUBCONFIG not in config_entry:
            self.config_entry = config_entry
            return

        sub = config_entry[SUBCONFIG]

        clone = config_entry.copy()
        clone.pop(SUBCONFIG)

        def verify_and_read_sub(sub: str, keep_only_keys):
            if not isinstance(sub, str):
                raise TypeError(f"Unknown type in {SUBCONFIG}")
            data = YamlFile.read_file(sub)
            if keep_only_keys is not None:
                return {k: data[k] for k in data if k in keep_only_keys}
            return data

        if isinstance(sub, str):
            replacement_sub: dict = verify_and_read_sub(sub, self.subconfig_filter)
            clone = {**clone, **replacement_sub}
        elif isinstance(sub, list):
            replacement_list = []
            for s in sub:
                replacement_list.append(verify_and_read_sub(s, self.subconfig_filter))
            replacement_sub = {SUBCONFIG: replacement_list}
            clone = {**clone, **replacement_sub}
        else:
            raise TypeError(f"{SUBCONFIG} is the wrong type: {type(sub)}")

        self.config_entry = clone
