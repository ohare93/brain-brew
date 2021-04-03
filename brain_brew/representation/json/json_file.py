import json

_encoding = "utf-8"


class JsonFile:
    @staticmethod
    def pretty_print(data):
        return json.dumps(data, indent=4)

    @staticmethod
    def to_filename_json(filename: str):
        if filename[-5:] != ".json":
            return filename + ".json"
        return filename

    @staticmethod
    def read_file(file_location):
        with open(JsonFile.to_filename_json(file_location), "r", encoding=_encoding) as read_file:
            return json.load(read_file)

    @staticmethod
    def write_file(file_location, data):
        with open(JsonFile.to_filename_json(file_location), "w+", encoding=_encoding) as write_file:
            json.dump(data, write_file, indent=4, sort_keys=False, ensure_ascii=False)
