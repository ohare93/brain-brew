use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::{
    AdapterIds, CanonicalDeck, CardTemplate, FieldDefinition, FieldImageReference, FieldValue,
    MediaReference, MessageComponent, Note, NoteType, SemanticChangeKind, StableId,
    StructuredMessage, TombstoneAddress, TombstoneRecord, Tombstones,
};

#[test]
fn unchanged_decks_have_empty_semantic_diff() {
    let left = ug_style_deck();
    let right = ug_style_deck();

    let diff = left.semantic_diff(&right);

    assert!(diff.is_empty());
}

#[test]
fn note_field_changes_are_reported_by_stable_id_path() {
    let left = ug_style_deck();
    let mut right = ug_style_deck();
    right
        .notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .fields
        .insert(sid("field.capital"), "Helsingfors".to_owned());

    let diff = left.semantic_diff(&right);

    assert_eq!(diff.changes.len(), 1);
    let change = &diff.changes[0];
    assert_eq!(change.kind, SemanticChangeKind::Modified);
    assert_eq!(change.path, "notes.note.finland.fields.field.capital");
    assert_eq!(change.before.as_deref(), Some("Helsinki"));
    assert_eq!(change.after.as_deref(), Some("Helsingfors"));
}

#[test]
fn structured_field_values_and_representation_changes_are_semantic_differences() {
    let left = ug_style_deck();
    let mut image = left.clone();
    image
        .notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .fields
        .insert(
            sid("field.flag"),
            FieldValue::Images(vec![FieldImageReference {
                media_id: sid("media.flag.finland"),
            }]),
        );
    assert!(image.semantic_diff(&left).has_change(
        SemanticChangeKind::Modified,
        "notes.note.finland.fields.field.flag"
    ));

    let mut message = left.clone();
    message
        .notes
        .get_mut(&sid("note.finland"))
        .unwrap()
        .fields
        .insert(
            sid("field.capital"),
            FieldValue::Message(StructuredMessage {
                components: vec![MessageComponent::Text("Helsinki".to_owned())],
                format: None,
                variables: BTreeMap::new(),
            }),
        );
    assert!(message.semantic_diff(&left).has_change(
        SemanticChangeKind::Modified,
        "notes.note.finland.fields.field.capital"
    ));
}

#[test]
fn added_and_removed_notes_are_reported_by_stable_id_not_position() {
    let left = ug_style_deck();
    let mut right = ug_style_deck();
    right.notes.remove(&sid("note.finland"));
    right.notes.insert(sid("note.sweden"), sweden_note());

    let diff = left.semantic_diff(&right);

    assert!(diff.has_change(SemanticChangeKind::Added, "notes.note.sweden"));
    assert!(diff.has_change(SemanticChangeKind::Removed, "notes.note.finland"));
}

#[test]
fn tombstones_are_distinct_semantic_changes() {
    let left = ug_style_deck();
    let mut right = ug_style_deck();
    right
        .tombstones
        .insert(TombstoneRecord::legacy(TombstoneAddress::Note {
            note_id: sid("note.finland"),
        }));

    let diff = left.semantic_diff(&right);

    assert!(diff.has_change(
        SemanticChangeKind::Tombstoned,
        "tombstones.notes.note.finland"
    ));
}

fn ug_style_deck() -> CanonicalDeck {
    let mut note_type_adapter_ids = AdapterIds::new();
    note_type_adapter_ids.insert("crowdanki:model_id", "1548959259107");

    let note_type = NoteType {
        id: sid("note-type.country"),
        name: "Ultimate Geography Country".to_owned(),
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
            id: sid("template.country-to-capital"),
            name: "Country - Capital".to_owned(),
            variables: BTreeMap::new(),
            question_format: "{{Country}}".to_owned(),
            answer_format: "{{FrontSide}}<hr id=answer>{{Capital}}".to_owned(),
            adapter_ids: AdapterIds::new(),
        }],
        styling: ".card { font-family: sans-serif; }\n".to_owned(),
        adapter_ids: note_type_adapter_ids,
    };

    let note = Note {
        id: sid("note.finland"),
        note_type_id: sid("note-type.country"),
        variables: BTreeMap::new(),
        fields: BTreeMap::from([
            (sid("field.country"), "Finland".to_owned()),
            (sid("field.capital"), "Helsinki".to_owned()),
            (sid("field.flag"), "<img src=\"fi.png\">".to_owned()),
        ])
        .into(),
        tags: BTreeSet::from(["Europe".to_owned(), "Nordic".to_owned()]),
        adapter_ids: AdapterIds::new(),
    };

    CanonicalDeck {
        id: sid("deck.ultimate-geography"),
        name: "Ultimate Geography".to_owned(),
        description: "A geography deck fixture.".to_owned(),
        variables: BTreeMap::new(),
        note_types: BTreeMap::from([(note_type.id.clone(), note_type)]),
        notes: BTreeMap::from([(note.id.clone(), note)]),
        media: BTreeMap::from([(
            sid("media.flag.finland"),
            MediaReference {
                id: sid("media.flag.finland"),
                path: "flags/fi.png".to_owned(),
                sha256: "0123456789abcdef".to_owned(),
            },
        )]),
        tombstones: Tombstones::default(),
        adapter_ids: AdapterIds::new(),
    }
}

fn sweden_note() -> Note {
    Note {
        id: sid("note.sweden"),
        note_type_id: sid("note-type.country"),
        variables: BTreeMap::new(),
        fields: BTreeMap::from([
            (sid("field.country"), "Sweden".to_owned()),
            (sid("field.capital"), "Stockholm".to_owned()),
            (sid("field.flag"), "<img src=\"se.png\">".to_owned()),
        ])
        .into(),
        tags: BTreeSet::from(["Europe".to_owned(), "Nordic".to_owned()]),
        adapter_ids: AdapterIds::new(),
    }
}

fn sid(value: &str) -> StableId {
    StableId::new(value).expect("test stable id is valid")
}
