use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::{
    AdapterIds, CanonicalDeck, CardTemplate, ChangeIntent, FieldDefinition, FieldImageReference,
    FieldValue, MediaReference, MessageComponent, Note, NoteType, RemovalProvenance,
    SemanticChange, SemanticChangeKind, StableId, StructuredMessage, TombstoneAddress,
    TombstoneRecord, Tombstones,
};

struct MutationCase {
    property: &'static str,
    expected: Vec<(SemanticChangeKind, &'static str)>,
    mutate: Box<dyn Fn(&mut CanonicalDeck)>,
}

#[test]
fn single_property_mutation_matrix_has_exact_paths_and_kinds() {
    let base = complete_deck();
    let cases = mutation_cases();
    assert_eq!(
        cases.len(),
        40,
        "update the declared inventory count with the table"
    );

    for case in cases {
        let mut changed = base.clone();
        (case.mutate)(&mut changed);
        let diff = base.semantic_diff(&changed);
        let actual = diff
            .changes
            .iter()
            .map(|change| (change.kind, change.path.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(actual, case.expected, "property: {}", case.property);
        assert_ne!(
            base, changed,
            "mutation must change equality: {}",
            case.property
        );
    }
}

#[test]
fn every_typed_tombstone_address_and_provenance_is_semantic() {
    let base = complete_deck();
    let addresses = all_tombstone_addresses();
    assert_eq!(
        addresses.len(),
        22,
        "update the typed tombstone inventory count"
    );

    for address in addresses {
        assert_tombstone_variant_is_in_inventory(&address);
        let mut changed = base.clone();
        changed
            .tombstones
            .insert(TombstoneRecord::legacy(address.clone()));
        let diff = base.semantic_diff(&changed);
        assert_eq!(diff.changes.len(), 1, "address: {address}");
        assert_eq!(diff.changes[0].kind, SemanticChangeKind::Tombstoned);
        assert_eq!(diff.changes[0].path, format!("tombstones.{address}"));
        assert!(
            diff.changes[0]
                .after
                .as_deref()
                .unwrap()
                .contains(address.kind())
        );
    }

    let address = TombstoneAddress::NoteTag {
        note_id: sid("note.one"),
        tag: "old".to_owned(),
    };
    let mut left = base.clone();
    left.tombstones.insert(TombstoneRecord {
        address: address.clone(),
        provenance: Some(RemovalProvenance {
            overlay_id: sid("overlay.one"),
            operation: ChangeIntent::Remove,
        }),
    });
    let mut right = left.clone();
    right.tombstones.insert(TombstoneRecord {
        address,
        provenance: Some(RemovalProvenance {
            overlay_id: sid("overlay.two"),
            operation: ChangeIntent::Override,
        }),
    });
    let diff = left.semantic_diff(&right);
    assert_eq!(diff.changes.len(), 1);
    assert_eq!(diff.changes[0].kind, SemanticChangeKind::Modified);
    assert_eq!(diff.changes[0].path, "tombstones.notes.note.one.tags.old");
    assert!(
        diff.changes[0]
            .before
            .as_deref()
            .unwrap()
            .contains("overlay.one")
    );
    assert!(
        diff.changes[0]
            .after
            .as_deref()
            .unwrap()
            .contains("overlay.two")
    );
    assert!(
        diff.changes[0]
            .after
            .as_deref()
            .unwrap()
            .contains("override")
    );
}

#[test]
fn exact_diff_obeys_identity_determinism_inversion_and_sorting_laws() {
    let base = complete_deck();
    assert!(base.semantic_diff(&base).is_empty());

    let mut changed = base.clone();
    changed.name = "Changed".to_owned();
    changed.variables.remove("deck.variable");
    changed
        .adapter_ids
        .insert("adapter.new", "new adapter value");
    changed
        .notes
        .get_mut(&sid("note.one"))
        .unwrap()
        .tags
        .remove("old");
    changed
        .notes
        .get_mut(&sid("note.one"))
        .unwrap()
        .tags
        .insert("new".to_owned());
    changed.media.get_mut(&sid("media.one")).unwrap().path = "changed.png".to_owned();

    let first = base.semantic_diff(&changed);
    for _ in 0..8 {
        assert_eq!(first, base.semantic_diff(&changed));
    }
    assert!(
        first
            .changes
            .windows(2)
            .all(|pair| pair[0].path <= pair[1].path)
    );

    let reverse = changed.semantic_diff(&base);
    assert_eq!(first.changes.len(), reverse.changes.len());
    for forward in &first.changes {
        let backward = reverse
            .changes
            .iter()
            .find(|change| change.path == forward.path)
            .unwrap_or_else(|| panic!("missing inverse for {}", forward.path));
        assert_eq!(backward.kind, inverse_kind(forward.kind));
        assert_eq!(backward.before, forward.after);
        assert_eq!(backward.after, forward.before);
    }
}

#[test]
fn map_and_set_insertion_order_is_normalized_but_explicit_sequences_are_not() {
    let left = complete_deck();
    let mut right = complete_deck();

    right.variables = [
        ("z".to_owned(), "Z".to_owned()),
        ("deck.variable".to_owned(), "deck value".to_owned()),
    ]
    .into_iter()
    .collect();
    let mut normalized_left = left.clone();
    normalized_left
        .variables
        .insert("z".to_owned(), "Z".to_owned());
    right.notes.get_mut(&sid("note.one")).unwrap().tags =
        ["tag".to_owned(), "old".to_owned()].into_iter().collect();
    assert!(normalized_left.semantic_diff(&right).is_empty());

    right
        .note_types
        .get_mut(&sid("note-type.one"))
        .unwrap()
        .fields
        .swap(0, 1);
    let diff = normalized_left.semantic_diff(&right);
    assert_eq!(diff.changes.len(), 1);
    assert_eq!(diff.changes[0].path, "note_types.note-type.one.fields");
}

#[test]
fn representation_variants_that_lower_to_the_same_bytes_remain_distinct() {
    let mut scalar = complete_deck();
    scalar
        .notes
        .get_mut(&sid("note.one"))
        .unwrap()
        .fields
        .insert(
            sid("field.images"),
            FieldValue::Scalar("<img src=\"one.png\" /><img src=\"two.png\" />".to_owned()),
        );
    let structured = complete_deck();
    assert_eq!(
        scalar.render_variables().unwrap(),
        structured.render_variables().unwrap()
    );
    let diff = scalar.semantic_diff(&structured);
    assert_eq!(diff.changes.len(), 1);
    assert_eq!(diff.changes[0].path, "notes.note.one.fields.field.images");
    assert_ne!(diff.changes[0].before, diff.changes[0].after);

    let mut message = complete_deck();
    message
        .notes
        .get_mut(&sid("note.one"))
        .unwrap()
        .fields
        .insert(
            sid("field.scalar"),
            FieldValue::Message(StructuredMessage {
                components: vec![MessageComponent::Text("Alpha".to_owned())],
                format: None,
                variables: BTreeMap::new(),
            }),
        );
    assert!(!message.semantic_diff(&complete_deck()).is_empty());
}

#[test]
fn empty_diff_is_equivalent_to_exact_canonical_equality_for_the_inventory() {
    let base = complete_deck();
    assert_eq!(base.semantic_diff(&base).is_empty(), base == base);
    for case in mutation_cases() {
        let mut changed = base.clone();
        (case.mutate)(&mut changed);
        assert_eq!(
            base.semantic_diff(&changed).is_empty(),
            base == changed,
            "property: {}",
            case.property
        );
    }
}

fn mutation_cases() -> Vec<MutationCase> {
    use SemanticChangeKind::Modified;
    vec![
        case("deck.id", Modified, "deck.id", |deck| {
            deck.id = sid("deck.changed")
        }),
        case("deck.name", Modified, "deck.name", |deck| {
            deck.name.push('!')
        }),
        case("deck.description", Modified, "deck.description", |deck| {
            deck.description.push('!')
        }),
        case(
            "deck.variables value",
            Modified,
            "deck.variables.deck.variable",
            |deck| {
                deck.variables
                    .insert("deck.variable".to_owned(), "changed".to_owned());
            },
        ),
        case(
            "deck.adapter_ids value",
            Modified,
            "deck.adapter_ids.adapter.deck",
            |deck| {
                deck.adapter_ids.insert("adapter.deck", "changed");
            },
        ),
        MutationCase {
            property: "note type map-key identity",
            expected: vec![
                (SemanticChangeKind::Added, "note_types.note-type.changed"),
                (SemanticChangeKind::Removed, "note_types.note-type.one"),
            ],
            mutate: Box::new(|deck| {
                let value = deck.note_types.remove(&sid("note-type.one")).unwrap();
                deck.note_types.insert(sid("note-type.changed"), value);
            }),
        },
        case(
            "note type payload identity",
            Modified,
            "note_types.note-type.one.id",
            |deck| nt(deck).id = sid("note-type.payload"),
        ),
        case(
            "note type name",
            Modified,
            "note_types.note-type.one.name",
            |deck| nt(deck).name.push('!'),
        ),
        case(
            "note type variable",
            Modified,
            "note_types.note-type.one.variables.note_type.variable",
            |deck| {
                nt(deck)
                    .variables
                    .insert("note_type.variable".to_owned(), "changed".to_owned());
            },
        ),
        MutationCase {
            property: "field definition explicit order",
            expected: vec![(Modified, "note_types.note-type.one.fields")],
            mutate: Box::new(|deck| nt(deck).fields.swap(0, 1)),
        },
        case(
            "field definition name",
            Modified,
            "note_types.note-type.one.fields.field.scalar.name",
            |deck| nt(deck).fields[0].name.push('!'),
        ),
        MutationCase {
            property: "field definition stable identity",
            expected: vec![
                (
                    SemanticChangeKind::Added,
                    "note_types.note-type.one.fields.field.changed",
                ),
                (
                    SemanticChangeKind::Removed,
                    "note_types.note-type.one.fields.field.scalar",
                ),
            ],
            mutate: Box::new(|deck| nt(deck).fields[0].id = sid("field.changed")),
        },
        MutationCase {
            property: "card template explicit order",
            expected: vec![(Modified, "note_types.note-type.one.card_templates")],
            mutate: Box::new(|deck| nt(deck).card_templates.swap(0, 1)),
        },
        MutationCase {
            property: "card template stable identity",
            expected: vec![
                (
                    SemanticChangeKind::Added,
                    "note_types.note-type.one.card_templates.template.changed",
                ),
                (
                    SemanticChangeKind::Removed,
                    "note_types.note-type.one.card_templates.template.one",
                ),
            ],
            mutate: Box::new(|deck| template(deck).id = sid("template.changed")),
        },
        case(
            "template name",
            Modified,
            "note_types.note-type.one.card_templates.template.one.name",
            |deck| template(deck).name.push('!'),
        ),
        case(
            "template variable",
            Modified,
            "note_types.note-type.one.card_templates.template.one.variables.template.variable",
            |deck| {
                template(deck)
                    .variables
                    .insert("template.variable".to_owned(), "changed".to_owned());
            },
        ),
        case(
            "template question format",
            Modified,
            "note_types.note-type.one.card_templates.template.one.question_format",
            |deck| template(deck).question_format.push('!'),
        ),
        case(
            "template answer format",
            Modified,
            "note_types.note-type.one.card_templates.template.one.answer_format",
            |deck| template(deck).answer_format.push('!'),
        ),
        case(
            "template adapter ID",
            Modified,
            "note_types.note-type.one.card_templates.template.one.adapter_ids.adapter.template",
            |deck| {
                template(deck)
                    .adapter_ids
                    .insert("adapter.template", "changed");
            },
        ),
        case(
            "note type styling",
            Modified,
            "note_types.note-type.one.styling",
            |deck| nt(deck).styling.push('!'),
        ),
        case(
            "note type adapter ID",
            Modified,
            "note_types.note-type.one.adapter_ids.adapter.note_type",
            |deck| {
                nt(deck).adapter_ids.insert("adapter.note_type", "changed");
            },
        ),
        MutationCase {
            property: "note map-key identity",
            expected: vec![
                (SemanticChangeKind::Added, "notes.note.changed"),
                (SemanticChangeKind::Removed, "notes.note.one"),
            ],
            mutate: Box::new(|deck| {
                let value = deck.notes.remove(&sid("note.one")).unwrap();
                deck.notes.insert(sid("note.changed"), value);
            }),
        },
        case(
            "note payload identity",
            Modified,
            "notes.note.one.id",
            |deck| note(deck).id = sid("note.payload"),
        ),
        case(
            "note type reference",
            Modified,
            "notes.note.one.note_type_id",
            |deck| note(deck).note_type_id = sid("note-type.changed"),
        ),
        case(
            "note variable",
            Modified,
            "notes.note.one.variables.note.variable",
            |deck| {
                note(deck)
                    .variables
                    .insert("note.variable".to_owned(), "changed".to_owned());
            },
        ),
        case(
            "scalar field",
            Modified,
            "notes.note.one.fields.field.scalar",
            |deck| {
                note(deck).fields.insert(
                    sid("field.scalar"),
                    FieldValue::Scalar("changed".to_owned()),
                );
            },
        ),
        case(
            "image reference media identity",
            Modified,
            "notes.note.one.fields.field.images.images.0",
            |deck| {
                let FieldValue::Images(images) =
                    note(deck).fields.get_mut(&sid("field.images")).unwrap()
                else {
                    panic!()
                };
                images[0].media_id = sid("media.three");
            },
        ),
        MutationCase {
            property: "image reference explicit order",
            expected: vec![
                (Modified, "notes.note.one.fields.field.images.images.0"),
                (Modified, "notes.note.one.fields.field.images.images.1"),
            ],
            mutate: Box::new(|deck| {
                let FieldValue::Images(images) =
                    note(deck).fields.get_mut(&sid("field.images")).unwrap()
                else {
                    panic!()
                };
                images.swap(0, 1);
            }),
        },
        case(
            "literal message component",
            Modified,
            "notes.note.one.fields.field.composite.message.0",
            |deck| {
                positional_component(deck, 0)
                    .clone_from(&MessageComponent::Literal("changed".to_owned()))
            },
        ),
        case(
            "text message component",
            Modified,
            "notes.note.one.fields.field.composite.message.1",
            |deck| {
                positional_component(deck, 1)
                    .clone_from(&MessageComponent::Text("changed".to_owned()))
            },
        ),
        case(
            "field-ref message component",
            Modified,
            "notes.note.one.fields.field.composite.message.2",
            |deck| {
                positional_component(deck, 2).clone_from(&MessageComponent::FieldRef(
                    "notes.note.one.fields.field.formatted".to_owned(),
                ))
            },
        ),
        case(
            "message format",
            Modified,
            "notes.note.one.fields.field.formatted.message.format",
            |deck| formatted_message(deck).format = Some("changed {value}".to_owned()),
        ),
        case(
            "named message variable",
            Modified,
            "notes.note.one.fields.field.formatted.message.variables.value",
            |deck| {
                formatted_message(deck).variables.insert(
                    "value".to_owned(),
                    MessageComponent::Literal("changed".to_owned()),
                );
            },
        ),
        case(
            "note tag set member",
            SemanticChangeKind::Added,
            "notes.note.one.tags.new",
            |deck| {
                note(deck).tags.insert("new".to_owned());
            },
        ),
        case(
            "note adapter ID",
            Modified,
            "notes.note.one.adapter_ids.adapter.note",
            |deck| {
                note(deck).adapter_ids.insert("adapter.note", "changed");
            },
        ),
        MutationCase {
            property: "media map-key identity",
            expected: vec![
                (SemanticChangeKind::Added, "media.media.changed"),
                (SemanticChangeKind::Removed, "media.media.one"),
            ],
            mutate: Box::new(|deck| {
                let value = deck.media.remove(&sid("media.one")).unwrap();
                deck.media.insert(sid("media.changed"), value);
            }),
        },
        case(
            "media payload identity",
            Modified,
            "media.media.one.id",
            |deck| deck.media.get_mut(&sid("media.one")).unwrap().id = sid("media.payload"),
        ),
        case("media path", Modified, "media.media.one.path", |deck| {
            deck.media
                .get_mut(&sid("media.one"))
                .unwrap()
                .path
                .push('!')
        }),
        case("media hash", Modified, "media.media.one.sha256", |deck| {
            deck.media
                .get_mut(&sid("media.one"))
                .unwrap()
                .sha256
                .push('!')
        }),
        case(
            "tombstone provenance",
            SemanticChangeKind::Tombstoned,
            "tombstones.notes.note.one.tags.removed",
            |deck| {
                deck.tombstones
                    .insert(TombstoneRecord::legacy(TombstoneAddress::NoteTag {
                        note_id: sid("note.one"),
                        tag: "removed".to_owned(),
                    }));
            },
        ),
    ]
}

fn case(
    property: &'static str,
    kind: SemanticChangeKind,
    path: &'static str,
    mutate: impl Fn(&mut CanonicalDeck) + 'static,
) -> MutationCase {
    MutationCase {
        property,
        expected: vec![(kind, path)],
        mutate: Box::new(mutate),
    }
}

fn complete_deck() -> CanonicalDeck {
    let fields = vec![
        FieldDefinition {
            id: sid("field.scalar"),
            name: "Scalar".to_owned(),
            message_pattern: None,
        },
        FieldDefinition {
            id: sid("field.images"),
            name: "Images".to_owned(),
            message_pattern: None,
        },
        FieldDefinition {
            id: sid("field.composite"),
            name: "Message".to_owned(),
            message_pattern: None,
        },
        FieldDefinition {
            id: sid("field.formatted"),
            name: "Formatted".to_owned(),
            message_pattern: None,
        },
    ];
    let templates = vec![
        CardTemplate {
            id: sid("template.one"),
            name: "Template one".to_owned(),
            variables: BTreeMap::from([(
                "template.variable".to_owned(),
                "template value".to_owned(),
            )]),
            question_format: "{{Scalar}}".to_owned(),
            answer_format: "{{FrontSide}}".to_owned(),
            adapter_ids: adapter_ids("adapter.template", "template adapter"),
        },
        CardTemplate {
            id: sid("template.two"),
            name: "Template two".to_owned(),
            variables: BTreeMap::new(),
            question_format: "{{Images}}".to_owned(),
            answer_format: "{{Message}}".to_owned(),
            adapter_ids: AdapterIds::new(),
        },
    ];
    let note_type = NoteType {
        id: sid("note-type.one"),
        name: "Note type".to_owned(),
        variables: BTreeMap::from([(
            "note_type.variable".to_owned(),
            "note type value".to_owned(),
        )]),
        fields,
        card_templates: templates,
        styling: ".card {}".to_owned(),
        adapter_ids: adapter_ids("adapter.note_type", "note type adapter"),
    };
    let note = Note {
        id: sid("note.one"),
        note_type_id: sid("note-type.one"),
        variables: BTreeMap::from([("note.variable".to_owned(), "note value".to_owned())]),
        fields: BTreeMap::from([
            (sid("field.scalar"), FieldValue::Scalar("Alpha".to_owned())),
            (
                sid("field.images"),
                FieldValue::Images(vec![
                    FieldImageReference {
                        media_id: sid("media.one"),
                    },
                    FieldImageReference {
                        media_id: sid("media.two"),
                    },
                ]),
            ),
            (
                sid("field.composite"),
                FieldValue::Message(StructuredMessage {
                    components: vec![
                        MessageComponent::Literal("(".to_owned()),
                        MessageComponent::Text("translated".to_owned()),
                        MessageComponent::FieldRef("notes.note.one.fields.field.scalar".to_owned()),
                    ],
                    format: None,
                    variables: BTreeMap::new(),
                }),
            ),
            (
                sid("field.formatted"),
                FieldValue::Message(StructuredMessage {
                    components: Vec::new(),
                    format: Some("Value: {value}".to_owned()),
                    variables: BTreeMap::from([(
                        "value".to_owned(),
                        MessageComponent::Text("formatted".to_owned()),
                    )]),
                }),
            ),
        ])
        .into(),
        tags: BTreeSet::from(["old".to_owned(), "tag".to_owned()]),
        adapter_ids: adapter_ids("adapter.note", "note adapter"),
    };
    let media = [
        ("media.one", "one.png"),
        ("media.two", "two.png"),
        ("media.three", "three.png"),
    ]
    .into_iter()
    .map(|(id, path)| {
        (
            sid(id),
            MediaReference {
                id: sid(id),
                path: path.to_owned(),
                sha256: format!("hash-{id}"),
            },
        )
    })
    .collect();
    CanonicalDeck {
        id: sid("deck.one"),
        name: "Deck".to_owned(),
        description: "Description".to_owned(),
        variables: BTreeMap::from([("deck.variable".to_owned(), "deck value".to_owned())]),
        note_types: BTreeMap::from([(sid("note-type.one"), note_type)]),
        notes: BTreeMap::from([(sid("note.one"), note)]),
        media,
        tombstones: Tombstones::default(),
        adapter_ids: adapter_ids("adapter.deck", "deck adapter"),
    }
}

fn all_tombstone_addresses() -> Vec<TombstoneAddress> {
    let nt = sid("note-type.one");
    let field = sid("field.scalar");
    let template = sid("template.one");
    let note = sid("note.one");
    vec![
        TombstoneAddress::DeckName,
        TombstoneAddress::DeckDescription,
        TombstoneAddress::DeckVariable {
            key: "gone".to_owned(),
        },
        TombstoneAddress::DeckAdapterId {
            key: "gone".to_owned(),
        },
        TombstoneAddress::NoteType {
            note_type_id: nt.clone(),
        },
        TombstoneAddress::NoteTypeName {
            note_type_id: nt.clone(),
        },
        TombstoneAddress::NoteTypeVariable {
            note_type_id: nt.clone(),
            key: "gone".to_owned(),
        },
        TombstoneAddress::NoteTypeStyling {
            note_type_id: nt.clone(),
        },
        TombstoneAddress::NoteTypeAdapterId {
            note_type_id: nt.clone(),
            key: "gone".to_owned(),
        },
        TombstoneAddress::FieldDefinition {
            note_type_id: nt.clone(),
            field_id: field,
        },
        TombstoneAddress::CardTemplate {
            note_type_id: nt.clone(),
            template_id: template.clone(),
        },
        TombstoneAddress::CardTemplateName {
            note_type_id: nt.clone(),
            template_id: template.clone(),
        },
        TombstoneAddress::CardTemplateVariable {
            note_type_id: nt.clone(),
            template_id: template.clone(),
            key: "gone".to_owned(),
        },
        TombstoneAddress::CardTemplateQuestionFormat {
            note_type_id: nt.clone(),
            template_id: template.clone(),
        },
        TombstoneAddress::CardTemplateAnswerFormat {
            note_type_id: nt.clone(),
            template_id: template.clone(),
        },
        TombstoneAddress::CardTemplateAdapterId {
            note_type_id: nt,
            template_id: template,
            key: "gone".to_owned(),
        },
        TombstoneAddress::Note {
            note_id: note.clone(),
        },
        TombstoneAddress::NoteVariable {
            note_id: note.clone(),
            key: "gone".to_owned(),
        },
        TombstoneAddress::NoteField {
            note_id: note.clone(),
            field_id: sid("field.scalar"),
        },
        TombstoneAddress::NoteTag {
            note_id: note.clone(),
            tag: "gone".to_owned(),
        },
        TombstoneAddress::NoteAdapterId {
            note_id: note,
            key: "gone".to_owned(),
        },
        TombstoneAddress::MediaReference {
            media_id: sid("media.one"),
        },
    ]
}

fn assert_tombstone_variant_is_in_inventory(address: &TombstoneAddress) {
    match address {
        TombstoneAddress::DeckName
        | TombstoneAddress::DeckDescription
        | TombstoneAddress::DeckVariable { key: _ }
        | TombstoneAddress::DeckAdapterId { key: _ }
        | TombstoneAddress::NoteType { note_type_id: _ }
        | TombstoneAddress::NoteTypeName { note_type_id: _ }
        | TombstoneAddress::NoteTypeVariable {
            note_type_id: _,
            key: _,
        }
        | TombstoneAddress::NoteTypeStyling { note_type_id: _ }
        | TombstoneAddress::NoteTypeAdapterId {
            note_type_id: _,
            key: _,
        }
        | TombstoneAddress::FieldDefinition {
            note_type_id: _,
            field_id: _,
        }
        | TombstoneAddress::CardTemplate {
            note_type_id: _,
            template_id: _,
        }
        | TombstoneAddress::CardTemplateName {
            note_type_id: _,
            template_id: _,
        }
        | TombstoneAddress::CardTemplateVariable {
            note_type_id: _,
            template_id: _,
            key: _,
        }
        | TombstoneAddress::CardTemplateQuestionFormat {
            note_type_id: _,
            template_id: _,
        }
        | TombstoneAddress::CardTemplateAnswerFormat {
            note_type_id: _,
            template_id: _,
        }
        | TombstoneAddress::CardTemplateAdapterId {
            note_type_id: _,
            template_id: _,
            key: _,
        }
        | TombstoneAddress::Note { note_id: _ }
        | TombstoneAddress::NoteVariable { note_id: _, key: _ }
        | TombstoneAddress::NoteField {
            note_id: _,
            field_id: _,
        }
        | TombstoneAddress::NoteTag { note_id: _, tag: _ }
        | TombstoneAddress::NoteAdapterId { note_id: _, key: _ }
        | TombstoneAddress::MediaReference { media_id: _ } => {}
    }
}

fn inverse_kind(kind: SemanticChangeKind) -> SemanticChangeKind {
    match kind {
        SemanticChangeKind::Added => SemanticChangeKind::Removed,
        SemanticChangeKind::Removed => SemanticChangeKind::Added,
        SemanticChangeKind::Modified => SemanticChangeKind::Modified,
        SemanticChangeKind::Tombstoned => SemanticChangeKind::Removed,
    }
}

fn nt(deck: &mut CanonicalDeck) -> &mut NoteType {
    deck.note_types.get_mut(&sid("note-type.one")).unwrap()
}

fn template(deck: &mut CanonicalDeck) -> &mut CardTemplate {
    &mut nt(deck).card_templates[0]
}

fn note(deck: &mut CanonicalDeck) -> &mut Note {
    deck.notes.get_mut(&sid("note.one")).unwrap()
}

fn positional_component(deck: &mut CanonicalDeck, index: usize) -> &mut MessageComponent {
    let FieldValue::Message(message) = note(deck).fields.get_mut(&sid("field.composite")).unwrap()
    else {
        panic!()
    };
    &mut message.components[index]
}

fn formatted_message(deck: &mut CanonicalDeck) -> &mut StructuredMessage {
    let FieldValue::Message(message) = note(deck).fields.get_mut(&sid("field.formatted")).unwrap()
    else {
        panic!()
    };
    message
}

fn adapter_ids(key: &str, value: &str) -> AdapterIds {
    let mut ids = AdapterIds::new();
    ids.insert(key, value);
    ids
}

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

#[allow(dead_code)]
fn assert_change_has_diagnostic_values(change: &SemanticChange) {
    assert!(change.before.is_some() || change.after.is_some());
}
