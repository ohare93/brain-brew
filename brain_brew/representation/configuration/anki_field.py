class AnkiField:
    name: str
    anki_name: str
    default_value: any

    def __init__(self, anki_name, name=None, default_value=None):
        self.anki_name = anki_name
        self.name = name if name is not None else anki_name
        self.default_value = default_value

    def append_name_if_differs(self, dict_to_add_to: dict, value):
        if value != self.default_value:
            dict_to_add_to.setdefault(self.name, value)
