use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::{
    StaleTranslation, TargetAdaptation, TargetAdaptationIntent, TargetAdaptationOwnership,
    TranslationDictionary, TranslationDictionaryRepair, TranslationMutation,
    TranslationMutationErrorKind,
};

fn dictionary() -> TranslationDictionary {
    TranslationDictionary {
        direct: BTreeMap::from([("Before".to_owned(), "Før".to_owned())]),
        contextual: BTreeMap::new(),
        no_change: BTreeSet::new(),
        target_adaptations: BTreeMap::new(),
        stale_translations: Vec::new(),
        variables: BTreeMap::new(),
        adapter_ids: BTreeMap::new(),
        require_complete: false,
        ignore_paths: BTreeSet::new(),
    }
}

#[test]
fn mutations_are_compare_and_set_and_transactional() {
    let mut translations = dictionary();
    let before = translations.clone();

    let error = translations
        .apply_mutations(&[
            TranslationMutation::ReplaceDirect {
                source: "Before".to_owned(),
                expected_target: "Før".to_owned(),
                target: "Efter".to_owned(),
            },
            TranslationMutation::ReplaceDirect {
                source: "Missing".to_owned(),
                expected_target: "Missing target".to_owned(),
                target: "New target".to_owned(),
            },
        ])
        .expect_err("a missing expected key must reject the complete transaction");

    assert_eq!(error.kind, TranslationMutationErrorKind::MissingKey);
    assert_eq!(error.path, "translations.direct.Missing");
    assert_eq!(
        translations, before,
        "a rejected batch must not partially mutate"
    );

    let error = translations
        .apply_mutation(TranslationMutation::ReplaceDirect {
            source: "Before".to_owned(),
            expected_target: "Wrong target".to_owned(),
            target: "Efter".to_owned(),
        })
        .expect_err("a mismatching expected target must reject replacement");
    assert_eq!(
        error.kind,
        TranslationMutationErrorKind::ExpectedValueMismatch
    );
    assert_eq!(error.path, "translations.direct.Before");
    assert_eq!(translations, before);
}

#[test]
fn direct_blanks_require_an_explicit_path_scoped_adaptation_or_no_change() {
    let mut translations = dictionary();

    let error = translations
        .apply_mutation(TranslationMutation::InsertDirect {
            source: "Deleted sentence".to_owned(),
            target: String::new(),
        })
        .expect_err("a blank direct translation is an implicit deletion");
    assert_eq!(
        error.kind,
        TranslationMutationErrorKind::BlankFaithfulTranslation
    );
    assert_eq!(error.path, "translations.direct.Deleted sentence");

    translations
        .apply_mutations(&[
            TranslationMutation::MarkNoChange {
                source: "Before".to_owned(),
            },
            TranslationMutation::SetTargetAdaptation {
                path: "notes.note.one.fields.field.front".to_owned(),
                intent: TargetAdaptationIntent::Delete,
                ownership: TargetAdaptationOwnership::Translation,
                expected_source: "Deleted sentence".to_owned(),
                target: String::new(),
                reason: "intentional target deletion".to_owned(),
            },
        ])
        .expect("no-change and explicit path-scoped deletion are distinct valid decisions");

    assert!(translations.no_change.contains("Before"));
    assert!(!translations.direct.contains_key("Before"));
    assert_eq!(
        translations.target_adaptations["notes.note.one.fields.field.front"],
        TargetAdaptation {
            intent: TargetAdaptationIntent::Delete,
            ownership: TargetAdaptationOwnership::Translation,
            expected_source: "Deleted sentence".to_owned(),
            target: String::new(),
            reason: "intentional target deletion".to_owned(),
        }
    );
}

#[test]
fn typed_target_decision_errors_are_atomic() {
    let mut translations = dictionary();
    let before = translations.clone();

    let error = translations
        .apply_mutations(&[
            TranslationMutation::SetTargetAdaptation {
                path: "notes.note.one.fields.field.front".to_owned(),
                intent: TargetAdaptationIntent::Delete,
                ownership: TargetAdaptationOwnership::Translation,
                expected_source: "Source-only sentence".to_owned(),
                target: String::new(),
                reason: "not suitable for this target edition".to_owned(),
            },
            TranslationMutation::SetTargetAdaptation {
                path: "notes.note.two.fields.field.front".to_owned(),
                intent: TargetAdaptationIntent::Adapt,
                ownership: TargetAdaptationOwnership::Extension,
                expected_source: String::new(),
                target: "extension content".to_owned(),
                reason: "wrong owner".to_owned(),
            },
        ])
        .expect_err("extension-owned content is not a translation adaptation");

    assert_eq!(error.kind, TranslationMutationErrorKind::InvalidContext);
    assert_eq!(
        error.path,
        "target_adaptations.notes.note.two.fields.field.front.ownership"
    );
    assert_eq!(
        translations, before,
        "the rejected batch must not add the deletion"
    );
}

#[test]
fn repairs_and_stale_resolution_are_atomic_and_preserve_shadowing() {
    let mut translations = dictionary();
    translations.no_change.insert("Covered".to_owned());
    translations.stale_translations.push(StaleTranslation {
        old_source: "Old".to_owned(),
        new_source: "Covered".to_owned(),
        target: "Old target".to_owned(),
        context: None,
    });
    let resolved = translations
        .resolve_stale_translation_decision("Old", "Covered", None, Some("Replacement"))
        .expect("shadowed stale entry resolves by removal only");
    assert_eq!(resolved.target, "Old target");
    assert!(translations.stale_translations.is_empty());
    assert!(translations.no_change.contains("Covered"));
    assert!(!translations.direct.contains_key("Covered"));

    let before = translations.clone();
    let error = translations
        .apply_repairs(&[
            TranslationDictionaryRepair::RemoveDirect {
                source: "Before".to_owned(),
            },
            TranslationDictionaryRepair::RemoveNoChange {
                source: "Missing".to_owned(),
            },
        ])
        .expect_err("a missing repair target rejects the complete repair transaction");
    assert_eq!(error.kind, TranslationMutationErrorKind::MissingKey);
    assert_eq!(translations, before);

    translations
        .apply_repairs(&[
            TranslationDictionaryRepair::SetRequireComplete(false),
            TranslationDictionaryRepair::RemoveDirect {
                source: "Before".to_owned(),
            },
        ])
        .expect("independent repairs apply deterministically");
    assert!(!translations.require_complete);
    assert!(!translations.direct.contains_key("Before"));
}

#[test]
fn independent_command_sequences_have_the_same_canonical_result() {
    let direct = TranslationMutation::SetContextual {
        occurrence_path: "notes.note.one.fields.field.front".to_owned(),
        context: "notes.note.one".to_owned(),
        source: "Before".to_owned(),
        target: "Efter".to_owned(),
    };
    let adaptation = TranslationMutation::SetTargetAdaptation {
        path: "notes.note.two.fields.field.front".to_owned(),
        intent: TargetAdaptationIntent::Delete,
        ownership: TargetAdaptationOwnership::Translation,
        expected_source: "Other source".to_owned(),
        target: String::new(),
        reason: "intentional deletion".to_owned(),
    };
    let mut left = dictionary();
    let mut right = dictionary();
    left.apply_mutations(&[direct.clone(), adaptation.clone()])
        .expect("independent commands apply");
    right
        .apply_mutations(&[adaptation, direct])
        .expect("independent commands apply in reverse order");
    assert_eq!(left, right);
}

#[test]
fn contextual_ownership_and_stale_records_are_canonicalized_deterministically() {
    let mut translations = dictionary();
    translations.stale_translations = vec![
        StaleTranslation {
            old_source: "Z old".to_owned(),
            new_source: "Z new".to_owned(),
            target: "Z target".to_owned(),
            context: Some("notes.note.z".to_owned()),
        },
        StaleTranslation {
            old_source: "A old".to_owned(),
            new_source: "A new".to_owned(),
            target: "A target".to_owned(),
            context: Some("notes.note.a".to_owned()),
        },
    ];

    translations
        .apply_mutation(TranslationMutation::SetContextual {
            occurrence_path: "notes.note.one.fields.field.front".to_owned(),
            context: "notes.note.one".to_owned(),
            source: "Before".to_owned(),
            target: "Efter".to_owned(),
        })
        .expect("a contextual decision owns its occurrence without replacing the direct fallback");

    assert_eq!(translations.direct["Before"], "Før");
    assert_eq!(translations.contextual["notes.note.one"]["Before"], "Efter");
    assert_eq!(
        translations
            .stale_translations
            .iter()
            .map(|record| record.context.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("notes.note.a"), Some("notes.note.z")],
        "core normalizes stale records rather than relying on adapter ordering"
    );
}
