import pytest

from brain_brew.representation.json.deck_part_header import DeckPartHeader
from tests.test_files import TestFiles
from tests.test_helpers import global_config


class TestConstructor:
    @pytest.mark.parametrize("header_name", [
        TestFiles.Headers.FIRST,
        TestFiles.Headers.FIRST_FULL,
    ])
    def test_runs(self, header_name, global_config):
        file = DeckPartHeader(header_name)

        assert isinstance(file, DeckPartHeader)
        assert file.file_location == TestFiles.Headers.FIRST_FULL
        assert len(file.get_data().keys()) == 10

    def test_config_location_override(self, global_config):
        loc = "place_for_stuff/"
        filename = "what-a-great-file.json"

        global_config.deck_parts.headers = loc

        file = DeckPartHeader(filename, read_now=False, data_override={
            "__type__": "Deck",
            "crowdanki_uuid": "72ac74b8-0077-11ea-959e-d8cb8ac9abf0",
            "deck_config_uuid": "3cc64d85-e410-11e9-960e-d8cb8ac9abf0",
            "name": "LL::1. Vocab"
        })

        assert file.file_location == loc + filename


@pytest.fixture()
def dp_headers_test1(global_config):
    return DeckPartHeader.create(TestFiles.Headers.FIRST_FULL)
