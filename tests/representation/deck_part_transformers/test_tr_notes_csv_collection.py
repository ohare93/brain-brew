from textwrap import dedent
from unittest.mock import patch

from brain_brew.file_manager import FileManager
from brain_brew.representation.deck_part_transformers.tr_notes_csv_collection import TrNotesCsvCollection
from brain_brew.representation.yaml.my_yaml import yaml_dump, yaml_load
from tests.test_file_manager import get_new_file_manager
from tests.representation.configuration.test_global_config import global_config


nm_mappings = {
    "LL Word": dedent(f'''\
        note_model: LL Word
        columns_to_fields:
          guid: guid
          tags: tags
        
          english: Word
          danish: X Word
          danish audio: X Pronunciation (Recording and/or IPA)
          esperanto: Y Word
          esperanto audio: Y Pronunciation (Recording and/or IPA)
        personal_fields:
          - picture
          - extra
          - morphman_focusmorph
    '''),
    "LL Verb": dedent(f'''\
        note_model: LL Verb
        csv_columns_to_fields:
          guid: guid
          tags: tags
    
          english: Word
          danish: X Word
          danish audio: X Pronunciation (Recording and/or IPA)
          esperanto: Y Word
          esperanto audio: Y Pronunciation (Recording and/or IPA)
    
          present: Form Present
          past: Form Past
          present perfect: Form Perfect Present
        personal_fields:
          - picture
          - extra
          - morphman_focusmorph
    '''),
    "LL Noun": dedent(f'''\
        note_model: LL Noun
        csv_columns_to_fields:
          guid: guid
          tags: tags
    
          english: Word
          danish: X Word
          danish audio: X Pronunciation (Recording and/or IPA)
          esperanto: Y Word
          esperanto audio: Y Pronunciation (Recording and/or IPA)
    
          plural: Plural
          indefinite plural: Indefinite Plural
          definite plural: Definite Plural
        personal_fields:
          - picture
          - extra
          - morphman_focusmorph
    ''')
}

file_mappings = {
    "Main1": dedent(f'''\
        file: source/vocab/main.csv
        note_model: LL Word
        sort_by_columns: [english]
        reverse_sort: no
    '''),
    "Der1": dedent(f'''\
        file: source/vocab/derivatives/danish/danish_verbs.csv
        note_model: LL Verb
    '''),
    "Der2": dedent(f'''\
        file: source/vocab/derivatives/danish/danish_nouns.csv
        note_model: LL Noun
    ''')
}


class TestConstructor:
    test_tr_notes = dedent(f'''\
        name: csv_first_attempt
        # save_to_file: deckparts/notes/csv_first_attempt.yaml

        note_model_mappings:
          - note_model: LL Word
            columns_to_fields:
              guid: guid
              tags: tags

              english: Word
              danish: X Word
              danish audio: X Pronunciation (Recording and/or IPA)
              esperanto: Y Word
              esperanto audio: Y Pronunciation (Recording and/or IPA)
            personal_fields:
              - picture
              - extra
              - morphman_focusmorph
          - note_model: LL Verb
            columns_to_fields:
              guid: guid
              tags: tags

              english: Word
              danish: X Word
              danish audio: X Pronunciation (Recording and/or IPA)
              esperanto: Y Word
              esperanto audio: Y Pronunciation (Recording and/or IPA)

              present: Form Present
              past: Form Past
              present perfect: Form Perfect Present
            personal_fields:
              - picture
              - extra
              - morphman_focusmorph
          - note_model: LL Noun
            columns_to_fields:
              guid: guid
              tags: tags

              english: Word
              danish: X Word
              danish audio: X Pronunciation (Recording and/or IPA)
              esperanto: Y Word
              esperanto audio: Y Pronunciation (Recording and/or IPA)

              plural: Plural
              indefinite plural: Indefinite Plural
              definite plural: Definite Plural
            personal_fields:
              - picture
              - extra
              - morphman_focusmorph

        file_mappings:
          - file: source/vocab/main.csv
            note_model: LL Word
            sort_by_columns: [english]
            reverse_sort: no

            derivatives:
              - file: source/vocab/derivatives/danish/danish_verbs.csv
                note_model: LL Verb
              - file: source/vocab/derivatives/danish/danish_nouns.csv
                note_model: LL Noun
    ''')

    def test_runs(self, global_config):
        fm = get_new_file_manager()
        data = yaml_load.load(self.test_tr_notes)

        tr_notes = TrNotesCsvCollection.from_dict(data)

        assert isinstance(tr_notes, TrNotesCsvCollection)
