use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::*;

#[test]
fn scalar_message_chain_and_diamond_resolve_in_dependency_order_once() {
    let deck = graph_deck([
        ("field.a", scalar("A")),
        ("field.b", message([reference("field.a")])),
        ("field.c", message([reference("field.a")])),
        (
            "field.d",
            message([
                reference("field.b"),
                MessageComponent::Literal("+".to_owned()),
                reference("field.c"),
            ]),
        ),
    ]);

    let graph = deck
        .resolve_field_graph(|_, _, _| -> Result<String, String> {
            unreachable!("this graph has no images")
        })
        .expect("diamond resolves");

    assert_eq!(graph.get(&path("field.d")), Some("A+A"));
    assert_eq!(
        graph.order(),
        &[
            path("field.a"),
            path("field.b"),
            path("field.c"),
            path("field.d"),
        ]
    );
}

#[test]
fn mixed_image_message_chain_uses_the_explicit_lowerer_once() {
    let mut deck = graph_deck([
        (
            "field.a",
            FieldValue::Images(vec![FieldImageReference {
                media_id: sid("media.flag"),
            }]),
        ),
        (
            "field.b",
            message([
                reference("field.a"),
                MessageComponent::Literal("!".to_owned()),
            ]),
        ),
        (
            "field.c",
            message([reference("field.a"), reference("field.b")]),
        ),
    ]);
    deck.media.insert(
        sid("media.flag"),
        MediaReference {
            id: sid("media.flag"),
            path: "flags/a b.svg".to_owned(),
            sha256: "abc".to_owned(),
        },
    );
    let calls = Cell::new(0);

    let graph = deck
        .resolve_field_graph(|_, _, images| {
            calls.set(calls.get() + 1);
            assert_eq!(images[0].media_id, sid("media.flag"));
            Ok::<_, String>("<image>".to_owned())
        })
        .expect("mixed image/message chain resolves");

    assert_eq!(
        calls.get(),
        1,
        "the shared image dependency is lowered once"
    );
    assert_eq!(graph.get(&path("field.c")), Some("<image><image>!"));
    assert_eq!(
        deck.render_variables()
            .expect("adapter lowering resolves mixed chain")
            .notes[&sid("note.main")]
            .fields[&sid("field.c")],
        "<img src=\"flags/a%20b.svg\" /><img src=\"flags/a%20b.svg\" />!"
    );
}

#[test]
fn missing_definition_value_and_tombstone_have_typed_consuming_diagnostics() {
    let missing_definition = graph_deck([
        ("field.a", scalar("A")),
        ("field.b", message([reference("field.undefined")])),
    ]);
    assert_graph_error(
        &missing_definition,
        FieldGraphErrorKind::MissingFieldDefinition,
        "field.b",
        "field.undefined",
    );

    let mut missing_value = graph_deck([
        ("field.a", scalar("A")),
        ("field.b", message([reference("field.missing-value")])),
    ]);
    missing_value
        .note_types
        .get_mut(&sid("note-type.graph"))
        .unwrap()
        .fields
        .push(FieldDefinition {
            id: sid("field.missing-value"),
            name: "field.missing-value".to_owned(),
            rtl: false,
            message_pattern: None,
        });
    assert_graph_error(
        &missing_value,
        FieldGraphErrorKind::MissingFieldValue,
        "field.b",
        "field.missing-value",
    );

    let mut tombstoned = graph_deck([
        ("field.a", scalar("A")),
        ("field.b", message([reference("field.a")])),
    ]);
    tombstoned
        .tombstones
        .insert(TombstoneRecord::legacy(TombstoneAddress::NoteField {
            note_id: sid("note.main"),
            field_id: sid("field.a"),
        }));
    assert_graph_error(
        &tombstoned,
        FieldGraphErrorKind::TombstonedDependency,
        "field.b",
        "field.a",
    );
}

#[test]
fn undefined_message_variable_is_a_typed_graph_error() {
    let deck = graph_deck([(
        "field.message",
        FieldValue::Message(StructuredMessage {
            components: Vec::new(),
            format: Some("{missing}".to_owned()),
            variables: BTreeMap::new(),
        }),
    )]);

    let report = deck.validate().expect_err("undefined variable must fail");
    let details = report
        .errors
        .iter()
        .find_map(|error| error.field_graph_error.as_ref())
        .expect("typed graph details are retained");
    assert_eq!(details.kind, FieldGraphErrorKind::InvalidMessage);
    assert_eq!(details.note_id, sid("note.main"));
    assert_eq!(details.field_id, sid("field.message"));
    assert_eq!(
        details.consuming_path,
        format!("{}.message.format", path("field.message"))
    );
    assert!(details.message.contains("undefined variable \"missing\""));
}

#[test]
fn invalid_image_lowering_is_typed_and_never_replaced_with_empty_text() {
    let deck = graph_deck([(
        "field.image",
        FieldValue::Images(vec![FieldImageReference {
            media_id: sid("media.missing"),
        }]),
    )]);

    let report = deck
        .resolve_field_graph(|_, _, _| Err::<String, _>("adapter cannot represent image"))
        .expect_err("lowering failure must surface");
    let error = &report.errors[0];
    assert_eq!(error.kind, FieldGraphErrorKind::InvalidTargetRepresentation);
    assert_eq!(error.note_id, sid("note.main"));
    assert_eq!(error.field_id, sid("field.image"));
    assert_eq!(
        error.dependency.as_deref(),
        Some(path("field.image").as_str())
    );
    assert_eq!(error.representation, Some(FieldValueKind::Images));
    assert!(error.message.contains("adapter cannot represent image"));
}

#[test]
fn self_two_node_and_cycle_with_tail_have_canonical_closed_traces() {
    let cases = [
        (
            graph_deck([("field.a", message([reference("field.a")]))]),
            vec![path("field.a"), path("field.a")],
        ),
        (
            graph_deck([
                ("field.a", message([reference("field.b")])),
                ("field.b", message([reference("field.a")])),
            ]),
            vec![path("field.a"), path("field.b"), path("field.a")],
        ),
        (
            graph_deck([
                ("field.a", message([reference("field.b")])),
                ("field.b", message([reference("field.c")])),
                ("field.c", message([reference("field.d")])),
                ("field.d", message([reference("field.b")])),
            ]),
            vec![
                path("field.b"),
                path("field.c"),
                path("field.d"),
                path("field.b"),
            ],
        ),
    ];

    for (deck, expected_cycle) in cases {
        let first = deck
            .validate()
            .expect_err("cycle fails canonical validation");
        let second = deck
            .validate()
            .expect_err("cycle diagnostics are repeatable");
        assert_eq!(first, second);
        let error = first
            .errors
            .iter()
            .find_map(|error| error.field_graph_error.as_ref())
            .expect("validation retains typed graph details");
        assert_eq!(error.kind, FieldGraphErrorKind::Cycle);
        assert_eq!(error.cycle, expected_cycle);
        assert_eq!(error.cycle.first(), error.cycle.last());
    }
}

#[test]
fn overlays_and_translations_update_downstream_messages_from_final_semantic_values() {
    let base = graph_deck([
        ("field.a", scalar("old")),
        ("field.b", message([reference("field.a")])),
        ("field.c", message([reference("field.b")])),
    ]);
    let downstream_first = overlay_field_replace(
        "overlay.downstream",
        "field.c",
        message([
            MessageComponent::Literal("[".to_owned()),
            reference("field.b"),
            MessageComponent::Literal("]".to_owned()),
        ]),
        FieldValue::Message(
            match &base.notes[&sid("note.main")].fields[&sid("field.c")] {
                FieldValue::Message(message) => message.clone(),
                _ => unreachable!(),
            },
        ),
    );
    let upstream_later =
        overlay_field_replace("overlay.upstream", "field.a", scalar("new"), scalar("old"));

    let composed = base
        .compose(&[downstream_first, upstream_later])
        .expect("overlays compose");
    assert_eq!(
        composed
            .field_text(&sid("note.main"), &sid("field.c"))
            .expect("downstream resolves"),
        "[new]"
    );

    let translation = Overlay {
        id: sid("overlay.translation"),
        kind: OverlayKind::Translation,
        translations: Some(TranslationDictionary {
            direct: BTreeMap::from([("old".to_owned(), "translated".to_owned())]),
            ..TranslationDictionary::default()
        }),
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };
    let translated = base
        .compose(std::slice::from_ref(&translation))
        .expect("translation composes");
    assert_eq!(
        translated
            .field_text(&sid("note.main"), &sid("field.c"))
            .expect("translated downstream resolves"),
        "translated"
    );

    let mut post_translation_update = overlay_field_replace(
        "overlay.post-translation",
        "field.a",
        scalar("newest"),
        scalar("translated"),
    );
    post_translation_update
        .note_changes
        .get_mut(&sid("note.main"))
        .unwrap()
        .fields
        .get_mut(&sid("field.a"))
        .unwrap()
        .intent = ChangeIntent::Override;
    let translated_then_updated = base
        .compose(&[translation, post_translation_update])
        .expect("post-translation upstream override composes");
    assert_eq!(
        translated_then_updated
            .field_text(&sid("note.main"), &sid("field.c"))
            .expect("live downstream reference resolves"),
        "newest"
    );
}

#[test]
fn output_and_errors_are_invariant_to_repeated_insertion_order_and_unrelated_fields() {
    let permutations = [
        vec!["field.a", "field.b", "field.c", "field.d"],
        vec!["field.d", "field.c", "field.b", "field.a"],
        vec!["field.b", "field.d", "field.a", "field.c"],
        vec!["field.c", "field.a", "field.d", "field.b"],
    ];
    let mut resolved_baseline = None;
    let mut error_baseline = None;

    for (iteration, order) in permutations.iter().cycle().take(32).enumerate() {
        let mut values = BTreeMap::new();
        for field in order {
            let value = match *field {
                "field.a" => scalar("A"),
                "field.b" => message([reference("field.a")]),
                "field.c" => message([reference("field.a")]),
                "field.d" => message([reference("field.b"), reference("field.c")]),
                _ => unreachable!(),
            };
            values.insert(*field, value);
        }
        values.insert(
            if iteration % 2 == 0 {
                "field.unrelated-z"
            } else {
                "field.unrelated-y"
            },
            scalar("ignored"),
        );
        let deck = graph_deck(values);
        let resolved = deck
            .field_text(&sid("note.main"), &sid("field.d"))
            .expect("graph resolves");
        assert_eq!(resolved, "AA");
        resolved_baseline.get_or_insert(resolved.clone());
        assert_eq!(resolved_baseline.as_ref(), Some(&resolved));

        let cyclic = graph_deck([
            ("field.a", message([reference("field.b")])),
            ("field.b", message([reference("field.a")])),
            (
                if iteration % 2 == 0 {
                    "field.unrelated-z"
                } else {
                    "field.unrelated-y"
                },
                scalar("ignored"),
            ),
        ]);
        let report = cyclic.validate().expect_err("cycle fails");
        let graph_error = report
            .errors
            .iter()
            .find_map(|error| error.field_graph_error.clone())
            .unwrap();
        error_baseline.get_or_insert(graph_error.clone());
        assert_eq!(error_baseline.as_ref(), Some(&graph_error));
    }
}

fn assert_graph_error(
    deck: &CanonicalDeck,
    expected_kind: FieldGraphErrorKind,
    consuming_field: &str,
    dependency_field: &str,
) {
    let report = deck
        .validate()
        .expect_err("invalid graph must fail validation");
    let error = report
        .errors
        .iter()
        .filter_map(|error| error.field_graph_error.as_ref())
        .find(|error| error.kind == expected_kind)
        .expect("expected typed graph error");
    assert_eq!(error.note_id, sid("note.main"));
    assert_eq!(error.field_id, sid(consuming_field));
    assert_eq!(
        error.dependency.as_deref(),
        Some(path(dependency_field).as_str())
    );
    assert!(error.consuming_path.starts_with(&path(consuming_field)));
}

fn graph_deck<'a>(values: impl IntoIterator<Item = (&'a str, FieldValue)>) -> CanonicalDeck {
    let values = values.into_iter().collect::<BTreeMap<_, _>>();
    let fields = values
        .keys()
        .map(|field_id| FieldDefinition {
            id: sid(field_id),
            name: (*field_id).to_owned(),
            rtl: false,
            message_pattern: None,
        })
        .collect::<Vec<_>>();
    let note_fields = values
        .into_iter()
        .map(|(field_id, value)| (sid(field_id), value))
        .collect::<FieldMap>();
    CanonicalDeck {
        id: sid("deck.graph"),
        name: "Graph".to_owned(),
        description: String::new(),
        variables: BTreeMap::new(),
        note_types: BTreeMap::from([(
            sid("note-type.graph"),
            NoteType {
                id: sid("note-type.graph"),
                name: "Graph".to_owned(),
                variables: BTreeMap::new(),
                fields,
                card_templates: Vec::new(),
                styling: String::new(),
                adapter_ids: AdapterIds::new(),
            },
        )]),
        notes: BTreeMap::from([(
            sid("note.main"),
            Note {
                id: sid("note.main"),
                note_type_id: sid("note-type.graph"),
                variables: BTreeMap::new(),
                fields: note_fields,
                tags: BTreeSet::new(),
                adapter_ids: AdapterIds::new(),
            },
        )]),
        media: BTreeMap::new(),
        tombstones: Tombstones::default(),
        adapter_ids: AdapterIds::new(),
    }
}

fn overlay_field_replace(
    overlay_id: &str,
    field_id: &str,
    value: FieldValue,
    expected: FieldValue,
) -> Overlay {
    Overlay {
        id: sid(overlay_id),
        kind: OverlayKind::Patch,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::from([(
            sid("note.main"),
            NoteChange {
                intent: ChangeIntent::Merge,
                note: None,
                variables: BTreeMap::new(),
                fields: BTreeMap::from([(
                    sid(field_id),
                    FieldChange {
                        intent: ChangeIntent::Replace,
                        value: Some(value),
                        expected_base: Some(ExpectedBase::FieldValue(expected)),
                    },
                )]),
                tags: BTreeMap::new(),
                adapter_ids: BTreeMap::new(),
                expected_base: None,
            },
        )]),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    }
}

fn scalar(value: &str) -> FieldValue {
    FieldValue::Scalar(value.to_owned())
}

fn message<const N: usize>(components: [MessageComponent; N]) -> FieldValue {
    FieldValue::Message(StructuredMessage {
        components: components.into(),
        format: None,
        variables: BTreeMap::new(),
    })
}

fn reference(field_id: &str) -> MessageComponent {
    MessageComponent::FieldRef(path(field_id))
}

fn path(field_id: &str) -> String {
    format!("notes.note.main.fields.{field_id}")
}

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}
