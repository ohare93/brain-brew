from brain_brew.representation.json.json_file import JsonFile


def debug_write_to_target_json(data, json: JsonFile):
    json.set_data(data)
    json.write_file()
