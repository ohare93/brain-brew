from brain_brew.representation.yaml.deck_part_holder import DeckPartHolder


def debug_write_deck_part_to_file(deck_part, filepath: str):
    dp = DeckPartHolder("Blah", filepath, deck_part)
    dp.save_to_file = filepath
    dp.write_to_file()
