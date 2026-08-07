use std::collections::{BTreeMap, BTreeSet};

use brain_brew_formats::canonical_yaml;
use brain_brew_formats::core::{
    AdapterIds, CanonicalDeck, ChangeIntent, DeckChange, ExpectedBase, FieldChange,
    FieldDefinition, FieldValue, MediaReference, Note, NoteChange, NoteType, Overlay, OverlayKind,
    PropertyChange, StableId, StructuredMessage, Tombstones,
};
use brain_brew_formats::lockfile::{
    FederationLock, LockedPackage, LockedPackageMetadata, LockedSource, OriginalSource,
};
use brain_brew_formats::manifest::{
    BuildTarget, CrowdAnkiTargetExport, FederatedDeckManifest, MetadataCategory,
    OverlayManifestEntry, TargetExports, TranslationCoveragePolicy, TranslationProfile,
};
use brain_brew_formats::{lockfile, manifest, media_map};

const HOSTILE_VALUES: &[(&str, &str)] = &[
    ("cr", "before\rafter"),
    ("crlf", "before\r\nafter"),
    ("nel", "before\u{85}after"),
    ("line-separator", "before\u{2028}after"),
    ("paragraph-separator", "before\u{2029}after"),
    ("bom", "\u{feff}before"),
    ("multiline-control", "before\nafter\u{1}"),
];

const HOSTILE_KEYS: &[(&str, &str)] = &[
    ("comment-like", "# c"),
    ("sequence-like", "- d"),
    ("mapping-like", "a: b"),
    ("empty", ""),
    ("leading-space", " leading"),
    ("trailing-space", "trailing "),
];

#[test]
fn all_emitters_round_trip_hostile_values_and_are_idempotent() {
    for (case, value) in HOSTILE_VALUES {
        let deck = canonical_deck_with_value(value);
        let canonical_once = canonical_yaml::to_string(&deck)
            .unwrap_or_else(|error| panic!("{case}: canonical deck emits: {error}"));
        assert_eq!(
            canonical_yaml::from_str(&canonical_once).unwrap_or_else(|error| panic!(
                "{case}: canonical parses: {error}\n{canonical_once}"
            )),
            deck,
            "{case}: canonical round trip"
        );
        assert_eq!(
            canonical_yaml::format_str(&canonical_once).expect("canonical formats"),
            canonical_once,
            "{case}: canonical idempotent"
        );

        let overlay = overlay_with_value(value);
        let overlay_once = canonical_yaml::overlay_to_string(&overlay).expect("overlay emits");
        assert_eq!(
            canonical_yaml::overlay_from_str(&overlay_once)
                .unwrap_or_else(|error| panic!("{case}: overlay parses: {error}\n{overlay_once}")),
            overlay,
            "{case}: overlay round trip"
        );
        assert_eq!(
            canonical_yaml::overlay_format_str(&overlay_once).expect("overlay formats"),
            overlay_once,
            "{case}: overlay idempotent"
        );

        let manifest = manifest_with_value(value);
        let manifest_once = manifest::to_string(&manifest).expect("manifest emits");
        assert_eq!(
            manifest::from_str(&manifest_once).unwrap_or_else(|error| panic!(
                "{case}: manifest parses: {error}\n{manifest_once}"
            )),
            manifest,
            "{case}: manifest round trip"
        );
        assert_eq!(
            manifest::format_str(&manifest_once).expect("manifest formats"),
            manifest_once,
            "{case}: manifest idempotent"
        );

        let lock = lockfile_with_value(value);
        let lock_once = lockfile::to_string(&lock).expect("lockfile emits");
        assert_eq!(
            lockfile::from_str(&lock_once)
                .unwrap_or_else(|error| panic!("{case}: lock parses: {error}\n{lock_once}")),
            lock,
            "{case}: lock round trip"
        );
        assert_eq!(
            lockfile::format_str(&lock_once).expect("lock formats"),
            lock_once,
            "{case}: lock idempotent"
        );

        let media = media_map_with_value(value);
        let media_once = media_map::to_string(&media);
        assert_eq!(
            media_map::from_str(&media_once)
                .unwrap_or_else(|error| panic!("{case}: media map parses: {error}\n{media_once}")),
            media,
            "{case}: media map round trip"
        );
        assert_eq!(
            media_map::format_str(&media_once).expect("media map formats"),
            media_once,
            "{case}: media map idempotent"
        );
    }
}

#[test]
fn hostile_string_map_keys_are_quoted_and_round_trip_where_the_schema_allows_them() {
    for (case, key) in HOSTILE_KEYS {
        let deck = canonical_deck_with_variable_key(key);
        let canonical_once = canonical_yaml::to_string(&deck)
            .unwrap_or_else(|error| panic!("{case}: canonical deck emits: {error}"));
        assert_eq!(canonical_yaml::from_str(&canonical_once).unwrap(), deck);
        assert_eq!(
            canonical_yaml::format_str(&canonical_once).unwrap(),
            canonical_once
        );

        let overlay = overlay_with_variable_key(key);
        let overlay_once = canonical_yaml::overlay_to_string(&overlay).expect("overlay emits");
        assert_eq!(
            canonical_yaml::overlay_from_str(&overlay_once).unwrap(),
            overlay
        );
        assert_eq!(
            canonical_yaml::overlay_format_str(&overlay_once).unwrap(),
            overlay_once
        );

        let manifest = manifest_with_map_key(key);
        let manifest_once = manifest::to_string(&manifest).expect("manifest emits");
        assert_eq!(manifest::from_str(&manifest_once).unwrap(), manifest);
        assert_eq!(manifest::format_str(&manifest_once).unwrap(), manifest_once);
    }
}

#[test]
fn newline_containing_map_keys_are_rejected_cleanly() {
    let error = canonical_yaml::to_string(&canonical_deck_with_variable_key("bad\nkey"))
        .expect_err("canonical deck rejects newline variable keys");
    assert!(error.to_string().contains("cannot be emitted safely"));

    let error = canonical_yaml::overlay_from_str(
        "id: overlay.hostile\nkind: patch\ndeck:\n  variables:\n    ? \"bad\\nkey\"\n    : intent: replace\n      value: value\n",
    )
    .expect_err("overlay rejects newline structural map keys");
    assert!(error.to_string().contains("cannot be emitted safely"));

    let error = manifest::from_str(
        "base: deck.yaml\noverlays:\n  ? \"bad\\nkey\"\n  : file: overlay.yaml\ntargets: {}\n",
    )
    .expect_err("manifest rejects newline map keys");
    assert!(error.to_string().contains("cannot be emitted safely"));

    let error = lockfile::from_str(
        "version: 2\npackages:\n  ? \"bad\\nkey\"\n  : manifest: brainbrew.yaml\n    package:\n      version: 1.0.0\n    original:\n      type: path\n      path: .\n    locked:\n      type: path\n      path: .\n      nar_hash: 'sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA='\n",
    )
    .expect_err("lockfile rejects newline package keys");
    assert!(error.to_string().contains("cannot be emitted safely"));

    let error = media_map::from_str("? \"bad\\nkey\"\n: path: media.png\n  sha256: abc\n")
        .expect_err("media map rejects newline stable id keys");
    assert!(error.to_string().contains("invalid stable id"));
}

#[test]
fn fallible_public_emitters_reject_constructible_invalid_map_keys() {
    let overlay_error = canonical_yaml::overlay_to_string(&overlay_with_variable_key("bad\nkey"))
        .expect_err("overlay emitter returns an error");
    assert!(
        overlay_error
            .to_string()
            .contains("cannot be emitted safely")
    );

    let mut conflicting_overlay = overlay_with_value("value");
    conflicting_overlay.note_changes.insert(
        sid("note.strict"),
        NoteChange {
            intent: ChangeIntent::Merge,
            note: None,
            variables: BTreeMap::new(),
            fields: BTreeMap::from([(
                sid("field.value"),
                FieldChange {
                    intent: ChangeIntent::Replace,
                    value: Some(FieldValue::Message(StructuredMessage {
                        components: Vec::new(),
                        format: None,
                        variables: BTreeMap::new(),
                    })),
                    expected_base: None,
                },
            )]),
            tags: BTreeMap::new(),
            adapter_ids: BTreeMap::new(),
            expected_base: None,
        },
    );
    let union_error = canonical_yaml::overlay_to_string(&conflicting_overlay)
        .expect_err("overlay emitter rejects conflicting field representations");
    assert!(
        union_error
            .to_string()
            .contains("notes.note.strict.fields.field.value")
    );

    let manifest_error = manifest::to_string(&manifest_with_map_key("bad\nkey"))
        .expect_err("manifest emitter returns an error");
    assert!(
        manifest_error
            .to_string()
            .contains("cannot be emitted safely")
    );

    let lock_error = lockfile::to_string(&lockfile_with_package_key("bad\nkey"))
        .expect_err("lockfile emitter returns an error");
    assert!(lock_error.to_string().contains("cannot be emitted safely"));
}

#[test]
fn unsafe_multiline_values_are_double_quoted_not_literal_blocks() {
    let cr = canonical_yaml::to_string(&canonical_deck_with_value("before\rafter"))
        .expect("canonical emits CR value");
    assert!(cr.contains("description: \"before\\rafter\""), "{cr}");
    assert!(!cr.contains("description: |"), "{cr}");

    let line_separator =
        canonical_yaml::to_string(&canonical_deck_with_value("before\u{2028}after"))
            .expect("canonical emits U+2028 value");
    assert!(
        line_separator.contains("description: \"before\\Lafter\""),
        "{line_separator}"
    );
    assert!(
        !line_separator.contains("description: |"),
        "{line_separator}"
    );
}

fn canonical_deck_with_value(value: &str) -> CanonicalDeck {
    let note_type = NoteType {
        id: sid("note-type.hostile"),
        name: value.to_owned(),
        variables: BTreeMap::new(),
        fields: vec![FieldDefinition {
            id: sid("field.value"),
            name: "Value".to_owned(),
            rtl: false,
            message_pattern: None,
        }],
        card_templates: vec![],
        styling: value.to_owned(),
        adapter_ids: AdapterIds::new(),
    };
    let note = Note {
        id: sid("note.hostile"),
        note_type_id: sid("note-type.hostile"),
        variables: BTreeMap::new(),
        fields: BTreeMap::from([(sid("field.value"), value.to_owned())]).into(),
        tags: BTreeSet::new(),
        adapter_ids: AdapterIds::new(),
    };
    CanonicalDeck {
        id: sid("deck.hostile"),
        name: "Hostile".to_owned(),
        description: value.to_owned(),
        variables: BTreeMap::new(),
        note_types: BTreeMap::from([(note_type.id.clone(), note_type)]),
        notes: BTreeMap::from([(note.id.clone(), note)]),
        media: media_map_with_value("media.png"),
        tombstones: Tombstones::default(),
        adapter_ids: AdapterIds::new(),
    }
}

fn canonical_deck_with_variable_key(key: &str) -> CanonicalDeck {
    let mut deck = canonical_deck_with_value("value");
    deck.variables
        .insert(key.to_owned(), "replacement".to_owned());
    deck
}

fn overlay_with_value(value: &str) -> Overlay {
    Overlay {
        id: sid("overlay.hostile"),
        kind: OverlayKind::Translation,
        translations: None,
        deck_change: Some(DeckChange {
            name: Some(PropertyChange {
                intent: ChangeIntent::Replace,
                value: Some(value.to_owned()),
                expected_base: Some(ExpectedBase::Value("old".to_owned())),
            }),
            description: None,
            variables: BTreeMap::new(),
            adapter_ids: BTreeMap::new(),
        }),
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    }
}

fn overlay_with_variable_key(key: &str) -> Overlay {
    let mut overlay = overlay_with_value("value");
    overlay.deck_change.as_mut().unwrap().variables.insert(
        key.to_owned(),
        PropertyChange {
            intent: ChangeIntent::Replace,
            value: Some("replacement".to_owned()),
            expected_base: None,
        },
    );
    overlay
}

fn manifest_with_value(value: &str) -> FederatedDeckManifest {
    FederatedDeckManifest {
        package: None,
        base: value.to_owned(),
        include_roots: vec![value.to_owned()],
        overlays: BTreeMap::from([(
            "overlay.hostile".to_owned(),
            OverlayManifestEntry {
                file: value.to_owned(),
                kind: Some("translation".to_owned()),
                depends_on: Vec::new(),
            },
        )]),
        targets: BTreeMap::from([(
            "target.hostile".to_owned(),
            BuildTarget {
                extends: Some(value.to_owned()),
                overlays: vec!["overlay.hostile".to_owned()],
                translation_coverage: TranslationCoveragePolicy::Strict,
                exports: TargetExports {
                    crowdanki: Some(CrowdAnkiTargetExport {
                        out: Some(value.to_owned()),
                        golden: Some(value.to_owned()),
                        golden_allowlist: vec![value.to_owned()],
                    }),
                },
            },
        )]),
        languages: BTreeMap::new(),
        translation_profile: TranslationProfile {
            structural_fields: Vec::new(),
            metadata_categories: vec![MetadataCategory {
                key: "category".to_owned(),
                label: value.to_owned(),
                paths: vec![value.to_owned()],
            }],
            metadata_paths: vec![value.to_owned()],
            metadata_exclude_paths: vec![value.to_owned()],
            metadata_category_order: vec!["category".to_owned()],
        },
    }
}

fn manifest_with_map_key(key: &str) -> FederatedDeckManifest {
    let mut manifest = manifest_with_value("value");
    manifest.overlays = BTreeMap::from([(
        key.to_owned(),
        OverlayManifestEntry {
            file: "overlay.yaml".to_owned(),
            kind: Some("translation".to_owned()),
            depends_on: Vec::new(),
        },
    )]);
    manifest.targets = BTreeMap::from([(
        key.to_owned(),
        BuildTarget {
            extends: None,
            overlays: vec![key.to_owned()],
            translation_coverage: TranslationCoveragePolicy::Lenient,
            exports: TargetExports::default(),
        },
    )]);
    manifest
}

fn lockfile_with_value(value: &str) -> FederationLock {
    FederationLock {
        version: 2,
        packages: BTreeMap::from([("package.hostile".to_owned(), locked_package(value))]),
    }
}

fn lockfile_with_package_key(key: &str) -> FederationLock {
    FederationLock {
        version: 2,
        packages: BTreeMap::from([(key.to_owned(), locked_package("value"))]),
    }
}

fn locked_package(value: &str) -> LockedPackage {
    LockedPackage {
        manifest: value.to_owned(),
        package: LockedPackageMetadata {
            version: "1.0.0".to_owned(),
        },
        original: OriginalSource::Path {
            path: value.to_owned(),
        },
        locked: locked_source(value),
    }
}

fn locked_source(value: &str) -> LockedSource {
    LockedSource::Path {
        path: value.to_owned(),
        nar_hash: "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_owned(),
    }
}

fn media_map_with_value(value: &str) -> BTreeMap<StableId, MediaReference> {
    BTreeMap::from([(
        sid("media.hostile"),
        MediaReference {
            id: sid("media.hostile"),
            path: value.to_owned(),
            sha256: value.to_owned(),
        },
    )])
}

fn sid(value: &str) -> StableId {
    StableId::new(value).expect("test stable id is valid")
}
