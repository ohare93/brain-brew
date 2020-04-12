

def blank_str_if_none(s):
    return '' if s is None else s


def list_of_str_to_lowercase(list_of_strings):
    return [entry.lower() for entry in list_of_strings]


def single_item_to_list(item):
    if isinstance(item, list):
        return item
    return [item]
