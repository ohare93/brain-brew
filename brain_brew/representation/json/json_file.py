import json

_encoding = "utf-8"


class JsonFile:
    @staticmethod
    def pretty_print(data):
        return json.dumps(data, indent=4)

    @staticmethod
    def read_file(file_location):
        with open(file_location, "r", encoding=_encoding) as read_file:
            return json.load(read_file)

    @staticmethod
    def write_file(file_location, data):
        with open(file_location, "w+", encoding=_encoding) as write_file:
            json.dump(data, write_file, indent=4, sort_keys=False, ensure_ascii=False)
