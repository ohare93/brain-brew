from unittest.mock import patch

import pytest

from brain_brew.constants.deckpart_keys import NoteFlagKeys
from brain_brew.constants.global_config_keys import ConfigKeys

from brain_brew.representation.configuration.global_config import GlobalConfig
from tests.test_files import TestFiles


class TestSingletonConstructor:
    def test_runs(self):
        fm = GlobalConfig.get_instance()
        assert isinstance(fm, GlobalConfig)

    def test_returns_existing_singleton(self):
        fm = GlobalConfig.get_instance()
        fm.known_files_dict = {'test': None}
        fm2 = GlobalConfig.get_instance()

        assert fm2.known_files_dict == {'test': None}
        assert fm2 == fm

    def test_raises_error(self):
        with pytest.raises(Exception):
            GlobalConfig({})
            GlobalConfig({})


@pytest.fixture()
def global_config():
    GlobalConfig.clear_instance()
    return GlobalConfig({
        ConfigKeys.DECK_PARTS.value: {
            ConfigKeys.HEADERS.value: TestFiles.Headers.LOC,
            ConfigKeys.NOTE_MODELS.value: TestFiles.NoteModels.LOC,
            ConfigKeys.NOTES.value: TestFiles.NoteFiles.LOC,
            ConfigKeys.MEDIA_FILES.value: TestFiles.MediaFiles.LOC,

            ConfigKeys.DECK_PARTS_NOTES_STRUCTURE.value: {
                NoteFlagKeys.GROUP_BY_NOTE_MODEL.value: False,
                NoteFlagKeys.EXTRACT_SHARED_TAGS.value: False
            }
        },
        ConfigKeys.FLAGS.value: {

        }
    })
