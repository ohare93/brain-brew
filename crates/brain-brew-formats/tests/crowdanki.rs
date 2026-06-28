use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::{
    AdapterIds, CanonicalDeck, CardTemplate, FieldDefinition, MediaReference, Note, NoteType,
    StableId,
};
use brain_brew_formats::crowdanki;

#[test]
fn exports_deterministic_crowdanki_json_preserving_adapter_identities() {
    let export = crowdanki::export_deck(&ug_style_deck()).expect("deck exports");

    let actual_json: serde_json::Value = serde_json::from_str(&export.deck_json).unwrap();
    let expected_json: serde_json::Value = serde_json::from_str(EXPECTED_CROWDANKI_JSON).unwrap();
    assert_eq!(actual_json, expected_json);
    assert_eq!(
        export.deck_json,
        crowdanki::export_deck(&ug_style_deck())
            .expect("second export succeeds")
            .deck_json
    );
    assert!(export.omitted_tombstones.is_empty());
}

#[test]
fn import_export_round_trip_is_semantically_equal_when_suggested_ids_match_source() {
    let original = ug_style_deck();
    let export = crowdanki::export_deck(&original).expect("deck exports");

    let imported = crowdanki::import_deck_accept_suggested_ids(&export.deck_json)
        .expect("exported CrowdAnki imports");

    assert!(original.semantic_diff(&imported).is_empty());
}

#[test]
fn import_preserves_crowdanki_adapter_identities() {
    let imported = crowdanki::import_deck_accept_suggested_ids(EXPECTED_CROWDANKI_JSON)
        .expect("CrowdAnki imports");

    assert_eq!(
        imported.adapter_ids.get("crowdanki:uuid"),
        Some("43c5ba66-9a65-11e8-90c9-a0481cc15658")
    );
    assert_eq!(
        imported.adapter_ids.get("crowdanki:deck_config_uuid"),
        Some("deck.ultimate-geography:deck-config")
    );
    assert_eq!(
        imported.adapter_ids.get("crowdanki:deck_config_name"),
        Some("Ultimate Geography")
    );
    assert_eq!(
        imported
            .note_types
            .get(&sid("note-type.country"))
            .unwrap()
            .adapter_ids
            .get("crowdanki:uuid"),
        Some("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")
    );
    assert_eq!(
        imported
            .notes
            .get(&sid("note.finland"))
            .unwrap()
            .adapter_ids
            .get("crowdanki:guid"),
        Some("ug-finland-guid")
    );
}

#[test]
fn crowdanki_parity_comparator_accepts_exact_match() {
    let expected: serde_json::Value = serde_json::json!({
        "name": "Deck",
        "notes": [{"guid": "abc", "fields": ["A"]}]
    });
    let actual = expected.clone();

    crowdanki::compare_deck_json_values(
        &expected,
        &actual,
        &crowdanki::CrowdAnkiParityOptions::default(),
    )
    .expect("exact JSON matches");
}

#[test]
fn crowdanki_parity_comparator_reports_json_paths() {
    let expected: serde_json::Value = serde_json::json!({
        "desc": "Expected description",
        "name": "Deck",
        "notes": [{"guid": "abc", "fields": ["A"]}]
    });
    let actual: serde_json::Value = serde_json::json!({
        "name": "Deck",
        "notes": [{"guid": "abc", "fields": ["B"]}],
        "deck_config_uuid": "legacy-default"
    });

    let report = crowdanki::compare_deck_json_values(
        &expected,
        &actual,
        &crowdanki::CrowdAnkiParityOptions::default(),
    )
    .expect_err("differences are reported");

    assert!(
        report
            .differences
            .iter()
            .any(|difference| difference.path == "$.deck_config_uuid")
    );
    assert!(
        report
            .differences
            .iter()
            .any(|difference| difference.path == "$.desc")
    );
    assert!(
        report
            .differences
            .iter()
            .any(|difference| difference.path == "$.notes[guid=\"abc\"].fields[0]")
    );
}

#[test]
fn crowdanki_parity_comparator_accepts_allowlisted_paths() {
    let expected: serde_json::Value = serde_json::json!({
        "name": "Deck",
        "notes": [{"guid": "abc", "fields": ["A"]}]
    });
    let actual: serde_json::Value = serde_json::json!({
        "name": "Deck",
        "notes": [{"guid": "abc", "fields": ["A"], "flags": 0}],
        "deck_config_uuid": "legacy-default"
    });
    let options = crowdanki::CrowdAnkiParityOptions {
        allowed_path_globs: BTreeSet::from([
            "$.deck_config_uuid".to_owned(),
            "$.notes[*].flags".to_owned(),
        ]),
    };

    crowdanki::compare_deck_json_values(&expected, &actual, &options)
        .expect("allowlisted JSON paths may differ");
}

#[test]
fn crowdanki_parity_comparator_matches_reordered_identity_arrays() {
    let expected: serde_json::Value = serde_json::json!({
        "notes": [
            {"guid": "a", "fields": ["A"]},
            {"guid": "b", "fields": ["B"]}
        ],
        "note_models": [{
            "crowdanki_uuid": "model-1",
            "flds": [{"name": "Country"}, {"name": "Capital"}],
            "tmpls": [{"ord": 0, "name": "A"}, {"ord": 1, "name": "B"}]
        }]
    });
    let actual: serde_json::Value = serde_json::json!({
        "note_models": [{
            "crowdanki_uuid": "model-1",
            "tmpls": [{"ord": 1, "name": "B"}, {"ord": 0, "name": "A"}],
            "flds": [{"name": "Capital"}, {"name": "Country"}]
        }],
        "notes": [
            {"guid": "b", "fields": ["B"]},
            {"guid": "a", "fields": ["A"]}
        ]
    });

    crowdanki::compare_deck_json_values(
        &expected,
        &actual,
        &crowdanki::CrowdAnkiParityOptions::default(),
    )
    .expect("identity-keyed arrays may reorder without a parity difference");
}

#[test]
fn crowdanki_parity_report_summarizes_repeated_defaults_and_serializes_to_json() {
    let expected: serde_json::Value = serde_json::json!({
        "notes": [
            {"guid": "a", "fields": ["A"]},
            {"guid": "b", "fields": ["B"]},
            {"guid": "c", "fields": ["C"]}
        ]
    });
    let actual: serde_json::Value = serde_json::json!({
        "notes": [
            {"guid": "a", "fields": ["A"], "flags": 0},
            {"guid": "b", "fields": ["B"], "flags": 0},
            {"guid": "c", "fields": ["C"], "flags": 0}
        ]
    });

    let report = crowdanki::compare_deck_json_values(
        &expected,
        &actual,
        &crowdanki::CrowdAnkiParityOptions::default(),
    )
    .expect_err("extra defaults are reported");
    let human = report.to_string();
    assert!(human.contains("Repeated differences"));
    assert!(human.contains("3 × $.notes[*].flags"));

    let json = serde_json::to_value(&report).expect("report serializes");
    assert_eq!(json["differences"][0]["kind"], "extra_actual");
    assert_eq!(json["differences"].as_array().unwrap().len(), 3);
}

#[test]
fn export_omits_tombstoned_notes_and_reports_their_stable_ids() {
    let mut deck = ug_style_deck();
    deck.tombstones.insert(sid("note.finland"));

    let export = crowdanki::export_deck(&deck).expect("deck exports");

    assert_eq!(export.omitted_tombstones, vec![sid("note.finland")]);
    assert!(!export.deck_json.contains("ug-finland-guid"));
}

const EXPECTED_CROWDANKI_JSON: &str = r#"{
  "__type__": "Deck",
  "children": [],
  "crowdanki_uuid": "43c5ba66-9a65-11e8-90c9-a0481cc15658",
  "deck_config_uuid": "deck.ultimate-geography:deck-config",
  "deck_configurations": [
    {
      "__type__": "DeckConfig",
      "autoplay": false,
      "crowdanki_uuid": "deck.ultimate-geography:deck-config",
      "dyn": false,
      "lapse": {
        "delays": [
          10
        ],
        "leechAction": 0,
        "leechFails": 8,
        "minInt": 1,
        "mult": 0
      },
      "maxTaken": 60,
      "name": "Ultimate Geography",
      "new": {
        "bury": true,
        "delays": [
          1,
          10
        ],
        "initialFactor": 2500,
        "ints": [
          1,
          4,
          7
        ],
        "order": 0,
        "perDay": 15,
        "separate": true
      },
      "replayq": true,
      "rev": {
        "bury": true,
        "ease4": 1.3,
        "fuzz": 0.05,
        "ivlFct": 1,
        "maxIvl": 36500,
        "minSpace": 1,
        "perDay": 100
      },
      "timer": 0
    }
  ],
  "desc": "A geography deck fixture.",
  "dyn": 0,
  "extendNew": 10,
  "extendRev": 50,
  "media_files": [
    "flags/fi.png"
  ],
  "name": "Ultimate Geography",
  "note_models": [
    {
      "__type__": "NoteModel",
      "crowdanki_uuid": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
      "css": ".card { font-family: sans-serif; }\n",
      "flds": [
        {
          "font": "Arial",
          "media": [],
          "name": "Country",
          "ord": 0,
          "rtl": false,
          "size": 20,
          "sticky": false
        },
        {
          "font": "Arial",
          "media": [],
          "name": "Capital",
          "ord": 1,
          "rtl": false,
          "size": 20,
          "sticky": false
        },
        {
          "font": "Arial",
          "media": [],
          "name": "Flag",
          "ord": 2,
          "rtl": false,
          "size": 20,
          "sticky": false
        }
      ],
      "latexPost": "\\end{document}",
      "latexPre": "\\documentclass[12pt]{article}\n\\special{papersize=3in,5in}\n\\usepackage{amssymb,amsmath}\n\\pagestyle{empty}\n\\setlength{\\parindent}{0in}\n\\begin{document}\n",
      "latexsvg": false,
      "name": "Country",
      "req": [],
      "sortf": 0,
      "tags": [],
      "tmpls": [
        {
          "afmt": "{{FrontSide}}<hr id=answer>{{Capital}}",
          "bafmt": "",
          "bfont": "",
          "bqfmt": "",
          "bsize": 0,
          "did": null,
          "name": "Country - Capital",
          "ord": 0,
          "qfmt": "{{Country}}",
          "scratchPad": 0
        }
      ],
      "type": 0,
      "vers": []
    }
  ],
  "notes": [
    {
      "__type__": "Note",
      "data": "",
      "fields": [
        "Finland",
        "Helsinki",
        "<img src=\"fi.png\">"
      ],
      "flags": 0,
      "guid": "ug-finland-guid",
      "note_model_uuid": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
      "tags": [
        "Europe",
        "Nordic"
      ]
    }
  ]
}
"#;

fn ug_style_deck() -> CanonicalDeck {
    let mut deck_adapter_ids = AdapterIds::new();
    deck_adapter_ids.insert("crowdanki:uuid", "43c5ba66-9a65-11e8-90c9-a0481cc15658");

    let mut note_type_adapter_ids = AdapterIds::new();
    note_type_adapter_ids.insert("crowdanki:uuid", "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");

    let note_type = NoteType {
        id: sid("note-type.country"),
        name: "Country".to_owned(),
        variables: BTreeMap::new(),
        fields: vec![
            FieldDefinition {
                id: sid("field.country"),
                name: "Country".to_owned(),
            },
            FieldDefinition {
                id: sid("field.capital"),
                name: "Capital".to_owned(),
            },
            FieldDefinition {
                id: sid("field.flag"),
                name: "Flag".to_owned(),
            },
        ],
        card_templates: vec![CardTemplate {
            id: sid("template.country-capital"),
            name: "Country - Capital".to_owned(),
            variables: BTreeMap::new(),
            question_format: "{{Country}}".to_owned(),
            answer_format: "{{FrontSide}}<hr id=answer>{{Capital}}".to_owned(),
            adapter_ids: AdapterIds::new(),
        }],
        styling: ".card { font-family: sans-serif; }\n".to_owned(),
        adapter_ids: note_type_adapter_ids,
    };

    let mut note_adapter_ids = AdapterIds::new();
    note_adapter_ids.insert("crowdanki:guid", "ug-finland-guid");

    let note = Note {
        id: sid("note.finland"),
        note_type_id: sid("note-type.country"),
        variables: BTreeMap::new(),
        fields: BTreeMap::from([
            (sid("field.country"), "Finland".to_owned()),
            (sid("field.capital"), "Helsinki".to_owned()),
            (sid("field.flag"), "<img src=\"fi.png\">".to_owned()),
        ]),
        field_messages: BTreeMap::new(),
        tags: BTreeSet::from(["Europe".to_owned(), "Nordic".to_owned()]),
        adapter_ids: note_adapter_ids,
    };

    CanonicalDeck {
        id: sid("deck.ultimate-geography"),
        name: "Ultimate Geography".to_owned(),
        description: "A geography deck fixture.".to_owned(),
        variables: BTreeMap::new(),
        note_types: BTreeMap::from([(note_type.id.clone(), note_type)]),
        notes: BTreeMap::from([(note.id.clone(), note)]),
        media: BTreeMap::from([(
            sid("media.flags-fi-png"),
            MediaReference {
                id: sid("media.flags-fi-png"),
                path: "flags/fi.png".to_owned(),
                sha256: String::new(),
            },
        )]),
        tombstones: BTreeSet::new(),
        adapter_ids: deck_adapter_ids,
    }
}

fn sid(value: &str) -> StableId {
    StableId::new(value).expect("test stable id is valid")
}
