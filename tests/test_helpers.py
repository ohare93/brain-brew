from brain_brew.representation.yaml.part_holder import PartHolder


def debug_write_part_to_file(part, filepath: str):
    dp = PartHolder("Blah", filepath, part)
    dp.save_to_file = filepath
    dp.write_to_file()
