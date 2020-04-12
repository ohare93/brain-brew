from brain_brew.representation.configuration.global_config import GlobalConfig
from brain_brew.representation.json.json_file import JsonFile


class DeckPartHeader(JsonFile):

    @classmethod
    def formatted_file_location(cls, location):
        return cls.get_json_file_location(GlobalConfig.get_instance().deck_parts.headers, location)

    def __init__(self, location, read_now=True, data_override=None):
        super().__init__(
            self.formatted_file_location(location),
            read_now=read_now, data_override=data_override
        )
