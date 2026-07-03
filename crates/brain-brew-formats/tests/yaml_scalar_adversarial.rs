use std::collections::{BTreeMap, BTreeSet};

use brain_brew_formats::core::{
    AdapterIds, CanonicalDeck, CardTemplate, FieldDefinition, MediaReference, Note, NoteType,
    StableId,
};
use brain_brew_formats::lockfile::{
    FederationLock, LockedPackage, LockedPackageMetadata, LockedSource,
};
use brain_brew_formats::manifest::{
    BuildTarget, CrowdAnkiTargetExport, FederatedDeckManifest, MetadataCategory,
    OverlayManifestEntry, PackageMetadata, TargetExports, TranslationCoveragePolicy,
    TranslationProfile,
};
use brain_brew_formats::{canonical_yaml, lockfile, manifest};

const HOSTILE_STRINGS: &[(&str, &str)] = &[
    ("empty", ""),
    ("leading space", " leading"),
    ("trailing space", "trailing "),
    (
        "first line starts with space",
        " leading first line\nsecond line",
    ),
    ("line has trailing spaces", "first line  \nsecond line\t"),
    ("yaml yes", "yes"),
    ("yaml no", "no"),
    ("yaml null", "null"),
    ("yaml tilde", "~"),
    ("yaml float", "1.0"),
    ("yaml hex", "0x1F"),
    ("single quote", "L'Anse aux Meadows"),
    ("double quote", "said \"hello\""),
    ("colon space", "capital: Helsinki"),
    ("trailing colon", "label:"),
    ("hash", "text # not a comment"),
    ("accented", "Québec"),
    ("cjk", "日本語"),
    ("hebrew rtl", "עברית"),
    ("lone newline", "\n"),
];

#[test]
fn canonical_yaml_round_trips_hostile_scalar_and_block_values() {
    for (case, value) in HOSTILE_STRINGS {
        let original = canonical_deck_with_value(value);
        let emitted = canonical_yaml::to_string(&original)
            .unwrap_or_else(|error| panic!("{case}: canonical deck emits: {error}"));

        let parsed = canonical_yaml::from_str(&emitted).unwrap_or_else(|error| {
            panic!("{case}: emitted canonical YAML parses: {error}\n{emitted}")
        });

        assert_eq!(parsed.description, *value, "{case}: deck description");
        assert_eq!(
            parsed.note_types[&sid("note-type.hostile")].styling,
            *value,
            "{case}: note type styling"
        );
        assert_eq!(
            parsed.note_types[&sid("note-type.hostile")].card_templates[0].question_format,
            *value,
            "{case}: card template question_format"
        );
        assert_eq!(
            parsed.notes[&sid("note.hostile")].fields[&sid("field.value")],
            *value,
            "{case}: note field value"
        );
        assert!(
            parsed.semantic_diff(&original).is_empty(),
            "{case}: emitted YAML preserves canonical deck semantics"
        );
    }
}

#[test]
fn manifest_yaml_round_trips_hostile_scalars() {
    for (case, value) in HOSTILE_STRINGS {
        let original = manifest_with_value(value);
        let emitted = manifest::to_string(&original);
        let parsed = manifest::from_str(&emitted)
            .unwrap_or_else(|error| panic!("{case}: emitted manifest parses: {error}\n{emitted}"));

        assert_eq!(parsed, original, "{case}: manifest round trip");
    }
}

#[test]
fn lockfile_yaml_round_trips_hostile_scalars() {
    for (case, value) in HOSTILE_STRINGS {
        let original = lockfile_with_value(value);
        let emitted = lockfile::to_string(&original);
        let parsed = lockfile::from_str(&emitted)
            .unwrap_or_else(|error| panic!("{case}: emitted lockfile parses: {error}\n{emitted}"));

        assert_eq!(parsed, original, "{case}: lockfile round trip");
    }
}

#[test]
fn emitted_yaml_is_idempotent_for_hostile_unit_cases() {
    for (case, value) in HOSTILE_STRINGS {
        let canonical_once = canonical_yaml::to_string(&canonical_deck_with_value(value))
            .unwrap_or_else(|error| panic!("{case}: canonical deck emits: {error}"));
        assert_eq!(
            canonical_yaml::format_str(&canonical_once)
                .unwrap_or_else(|error| panic!("{case}: canonical YAML formats: {error}")),
            canonical_once,
            "{case}: canonical YAML format is byte-idempotent"
        );

        let manifest_once = manifest::to_string(&manifest_with_value(value));
        assert_eq!(
            manifest::format_str(&manifest_once)
                .unwrap_or_else(|error| panic!("{case}: manifest formats: {error}")),
            manifest_once,
            "{case}: manifest format is byte-idempotent"
        );

        let lockfile_once = lockfile::to_string(&lockfile_with_value(value));
        assert_eq!(
            lockfile::format_str(&lockfile_once)
                .unwrap_or_else(|error| panic!("{case}: lockfile formats: {error}")),
            lockfile_once,
            "{case}: lockfile format is byte-idempotent"
        );
    }
}

fn canonical_deck_with_value(value: &str) -> CanonicalDeck {
    let note_type = NoteType {
        id: sid("note-type.hostile"),
        name: "Hostile Scalars".to_owned(),
        variables: BTreeMap::new(),
        fields: vec![FieldDefinition {
            id: sid("field.value"),
            name: "Value".to_owned(),
        }],
        card_templates: vec![CardTemplate {
            id: sid("template.hostile"),
            name: "Hostile Template".to_owned(),
            variables: BTreeMap::new(),
            question_format: value.to_owned(),
            answer_format: "answer".to_owned(),
            adapter_ids: AdapterIds::new(),
        }],
        styling: value.to_owned(),
        adapter_ids: AdapterIds::new(),
    };

    let note = Note {
        id: sid("note.hostile"),
        note_type_id: sid("note-type.hostile"),
        variables: BTreeMap::new(),
        fields: BTreeMap::from([(sid("field.value"), value.to_owned())]),
        field_messages: BTreeMap::new(),
        tags: BTreeSet::new(),
        adapter_ids: AdapterIds::new(),
    };

    CanonicalDeck {
        id: sid("deck.hostile"),
        name: "Hostile Scalars".to_owned(),
        description: value.to_owned(),
        variables: BTreeMap::new(),
        note_types: BTreeMap::from([(note_type.id.clone(), note_type)]),
        notes: BTreeMap::from([(note.id.clone(), note)]),
        media: BTreeMap::from([(
            sid("media.hostile"),
            MediaReference {
                id: sid("media.hostile"),
                path: "media.png".to_owned(),
                sha256: "0123456789abcdef".to_owned(),
            },
        )]),
        tombstones: BTreeSet::new(),
        adapter_ids: AdapterIds::new(),
    }
}

fn manifest_with_value(value: &str) -> FederatedDeckManifest {
    FederatedDeckManifest {
        package: Some(PackageMetadata {
            id: value.to_owned(),
            version: value.to_owned(),
            compatible_base_versions: vec![value.to_owned()],
            depends_on: vec![value.to_owned()],
        }),
        base: value.to_owned(),
        include_roots: vec![value.to_owned()],
        overlays: BTreeMap::from([(
            "overlay.hostile".to_owned(),
            OverlayManifestEntry {
                file: value.to_owned(),
                kind: Some(value.to_owned()),
                depends_on: vec![value.to_owned()],
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
            structural_fields: vec![value.to_owned()],
            metadata_categories: vec![MetadataCategory {
                key: value.to_owned(),
                label: value.to_owned(),
                paths: vec![value.to_owned()],
            }],
            metadata_paths: vec![value.to_owned()],
            metadata_exclude_paths: vec![value.to_owned()],
            metadata_category_order: vec![value.to_owned()],
        },
    }
}

fn lockfile_with_value(value: &str) -> FederationLock {
    FederationLock {
        version: 1,
        packages: BTreeMap::from([(
            "package.hostile".to_owned(),
            LockedPackage {
                manifest: value.to_owned(),
                package: LockedPackageMetadata {
                    version: value.to_owned(),
                },
                original: Some(locked_source_with_value(value)),
                locked: locked_source_with_value(value),
            },
        )]),
    }
}

fn locked_source_with_value(value: &str) -> LockedSource {
    LockedSource {
        source_type: value.to_owned(),
        url: Some(value.to_owned()),
        path: Some(value.to_owned()),
        reference: Some(value.to_owned()),
        rev: Some(value.to_owned()),
        nar_hash: Some(value.to_owned()),
    }
}

fn sid(value: &str) -> StableId {
    StableId::new(value).expect("test stable id is valid")
}
