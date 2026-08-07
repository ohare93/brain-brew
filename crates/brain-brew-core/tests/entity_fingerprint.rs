use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use brain_brew_core::{
    AdapterIds, CardTemplate, EntityFingerprint, FieldDefinition, FieldImageReference, FieldValue,
    MediaReference, MessageComponent, Note, NoteType, StableId, StructuredMessage,
    fingerprint_card_template, fingerprint_field_definition, fingerprint_media_reference,
    fingerprint_note, fingerprint_note_type,
};

#[test]
fn fingerprint_text_is_canonical_and_validated() {
    for version in ["v1", "v2"] {
        let text = format!(
            "sha256:{version}:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
        assert_eq!(
            EntityFingerprint::from_str(&text)
                .expect("supported canonical fingerprint parses")
                .to_string(),
            text
        );
    }

    for invalid in [
        "sha1:v2:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "sha256:v3:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "sha256:v2:ABCDEF6789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "sha256:v2:short",
    ] {
        assert!(
            EntityFingerprint::from_str(invalid).is_err(),
            "{invalid} must fail closed"
        );
    }
}

#[test]
fn map_insertion_order_does_not_affect_fingerprints() {
    let fixtures = fixtures();
    let mut left = fixtures.template.clone();
    left.variables.clear();
    left.variables.insert("a".to_owned(), "one".to_owned());
    left.variables.insert("b".to_owned(), "two".to_owned());
    let mut right = fixtures.template;
    right.variables.clear();
    right.variables.insert("b".to_owned(), "two".to_owned());
    right.variables.insert("a".to_owned(), "one".to_owned());

    assert_eq!(
        fingerprint_card_template(&left),
        fingerprint_card_template(&right)
    );
}

#[test]
fn entity_fingerprint_golden_vectors_are_stable() {
    let fixtures = fixtures();
    assert_eq!(
        fingerprint_field_definition(&fixtures.field).to_string(),
        "sha256:v2:aac0189b585f5118de3fde39df4eb90fddf9ddcb8a5018c8090b5fe7f8b73183"
    );
    assert_eq!(
        fingerprint_card_template(&fixtures.template).to_string(),
        "sha256:v1:8dbf452465b4112b38232f181711b8d16ed51bfef205b69758296a266bd2f12f"
    );
    assert_eq!(
        fingerprint_note_type(&fixtures.note_type).to_string(),
        "sha256:v2:603bb75c87655dd62c76c8c1698ffdc643dfcd9e1584e95012a87aa5f75a9901"
    );
    assert_eq!(
        fingerprint_note(&fixtures.note).to_string(),
        "sha256:v1:ba209b923265028159f3987f1421d2e184fe15473a20650e198c8dbd4c261ca8"
    );
    assert_eq!(
        fingerprint_media_reference(&fixtures.media).to_string(),
        "sha256:v1:df7e827400943da2f15f327fa7fec60d1828d9346e71d7eaa3c0b13fc1f379ee"
    );
}

#[test]
fn every_semantic_entity_property_changes_its_fingerprint() {
    let fixtures = fixtures();

    let field = fingerprint_field_definition(&fixtures.field);
    let mut changed = fixtures.field.clone();
    changed.id = sid("field.changed");
    assert_ne!(field, fingerprint_field_definition(&changed));
    let mut changed = fixtures.field.clone();
    changed.name.push('!');
    assert_ne!(field, fingerprint_field_definition(&changed));
    let mut changed = fixtures.field.clone();
    changed.rtl = true;
    assert_ne!(field, fingerprint_field_definition(&changed));

    let template = fingerprint_card_template(&fixtures.template);
    let mut mutations = Vec::new();
    let mut changed = fixtures.template.clone();
    changed.id = sid("template.changed");
    mutations.push(changed);
    let mut changed = fixtures.template.clone();
    changed.name.push('!');
    mutations.push(changed);
    let mut changed = fixtures.template.clone();
    changed
        .variables
        .insert("a".to_owned(), "changed".to_owned());
    mutations.push(changed);
    let mut changed = fixtures.template.clone();
    changed.question_format.push('!');
    mutations.push(changed);
    let mut changed = fixtures.template.clone();
    changed.answer_format.push('!');
    mutations.push(changed);
    let mut changed = fixtures.template.clone();
    changed.adapter_ids.insert("anki:ord", "2");
    mutations.push(changed);
    for changed in mutations {
        assert_ne!(template, fingerprint_card_template(&changed));
    }

    let note_type = fingerprint_note_type(&fixtures.note_type);
    let mut mutations = Vec::new();
    let mut changed = fixtures.note_type.clone();
    changed.id = sid("note-type.changed");
    mutations.push(changed);
    let mut changed = fixtures.note_type.clone();
    changed.name.push('!');
    mutations.push(changed);
    let mut changed = fixtures.note_type.clone();
    changed
        .variables
        .insert("label".to_owned(), "Changed".to_owned());
    mutations.push(changed);
    let mut changed = fixtures.note_type.clone();
    changed.fields[0].name.push('!');
    mutations.push(changed);
    let mut changed = fixtures.note_type.clone();
    changed.fields.reverse();
    mutations.push(changed);
    let mut changed = fixtures.note_type.clone();
    changed.card_templates[0].name.push('!');
    mutations.push(changed);
    let mut changed = fixtures.note_type.clone();
    changed.card_templates.reverse();
    mutations.push(changed);
    let mut changed = fixtures.note_type.clone();
    changed.styling.push('!');
    mutations.push(changed);
    let mut changed = fixtures.note_type.clone();
    changed.adapter_ids.insert("anki:model", "changed");
    mutations.push(changed);
    for changed in mutations {
        assert_ne!(note_type, fingerprint_note_type(&changed));
    }

    let note = fingerprint_note(&fixtures.note);
    let mut mutations = Vec::new();
    let mut changed = fixtures.note.clone();
    changed.id = sid("note.changed");
    mutations.push(changed);
    let mut changed = fixtures.note.clone();
    changed.note_type_id = sid("note-type.changed");
    mutations.push(changed);
    let mut changed = fixtures.note.clone();
    changed
        .variables
        .insert("x".to_owned(), "changed".to_owned());
    mutations.push(changed);
    let mut changed = fixtures.note.clone();
    changed.fields.insert(sid("field.scalar"), "changed");
    mutations.push(changed);
    let mut changed = fixtures.note.clone();
    changed.fields.insert(
        sid("field.images"),
        FieldValue::Images(vec![FieldImageReference {
            media_id: sid("media.changed"),
        }]),
    );
    mutations.push(changed);
    let mut changed = fixtures.note.clone();
    let message = changed
        .fields
        .get_mut(&sid("field.message"))
        .unwrap()
        .as_message_mut()
        .unwrap();
    message.format.as_mut().unwrap().push('!');
    mutations.push(changed);
    let mut changed = fixtures.note.clone();
    let message = changed
        .fields
        .get_mut(&sid("field.message"))
        .unwrap()
        .as_message_mut()
        .unwrap();
    let component = message.variables.remove("name").unwrap();
    message
        .variables
        .insert("changed-name".to_owned(), component);
    mutations.push(changed);
    let mut changed = fixtures.note.clone();
    let message = changed
        .fields
        .get_mut(&sid("field.message"))
        .unwrap()
        .as_message_mut()
        .unwrap();
    message.variables.insert(
        "name".to_owned(),
        MessageComponent::Text("Changed".to_owned()),
    );
    mutations.push(changed);
    let mut changed = fixtures.note.clone();
    let message = changed
        .fields
        .get_mut(&sid("field.message"))
        .unwrap()
        .as_message_mut()
        .unwrap();
    message.variables.insert(
        "name".to_owned(),
        MessageComponent::Literal("Name".to_owned()),
    );
    mutations.push(changed);
    let mut changed = fixtures.note.clone();
    let message = changed
        .fields
        .get_mut(&sid("field.message"))
        .unwrap()
        .as_message_mut()
        .unwrap();
    message.variables.insert(
        "value".to_owned(),
        MessageComponent::FieldRef("notes.note.changed.fields.field.scalar".to_owned()),
    );
    mutations.push(changed);
    let mut changed = fixtures.note.clone();
    changed.tags.insert("Changed".to_owned());
    mutations.push(changed);
    let mut changed = fixtures.note.clone();
    changed.adapter_ids.insert("anki:guid", "changed");
    mutations.push(changed);
    for changed in mutations {
        assert_ne!(note, fingerprint_note(&changed));
    }

    let mut positional = fixtures.note.clone();
    positional.fields.insert(
        sid("field.message"),
        FieldValue::Message(StructuredMessage {
            components: vec![
                MessageComponent::Literal("(".to_owned()),
                MessageComponent::Text("hello".to_owned()),
                MessageComponent::FieldRef("notes.note.one.fields.field.scalar".to_owned()),
            ],
            format: None,
            variables: BTreeMap::new(),
        }),
    );
    let positional_fingerprint = fingerprint_note(&positional);
    let mut changed = positional.clone();
    changed
        .fields
        .get_mut(&sid("field.message"))
        .unwrap()
        .as_message_mut()
        .unwrap()
        .components
        .reverse();
    assert_ne!(positional_fingerprint, fingerprint_note(&changed));

    let mut image_sequence = fixtures.note.clone();
    image_sequence.fields.insert(
        sid("field.images"),
        FieldValue::Images(vec![
            FieldImageReference {
                media_id: sid("media.one"),
            },
            FieldImageReference {
                media_id: sid("media.two"),
            },
        ]),
    );
    let image_fingerprint = fingerprint_note(&image_sequence);
    let FieldValue::Images(images) = image_sequence.fields.get_mut(&sid("field.images")).unwrap()
    else {
        unreachable!()
    };
    images.reverse();
    assert_ne!(image_fingerprint, fingerprint_note(&image_sequence));

    let media = fingerprint_media_reference(&fixtures.media);
    let mut changed = fixtures.media.clone();
    changed.id = sid("media.changed");
    assert_ne!(media, fingerprint_media_reference(&changed));
    let mut changed = fixtures.media.clone();
    changed.path.push('!');
    assert_ne!(media, fingerprint_media_reference(&changed));
    let mut changed = fixtures.media.clone();
    changed.sha256.push('!');
    assert_ne!(media, fingerprint_media_reference(&changed));
}

struct Fixtures {
    field: FieldDefinition,
    template: CardTemplate,
    note_type: NoteType,
    note: Note,
    media: MediaReference,
}

fn fixtures() -> Fixtures {
    let field = FieldDefinition {
        id: sid("field.front"),
        name: "Front".to_owned(),
        rtl: false,
        message_pattern: None,
    };
    let second_field = FieldDefinition {
        id: sid("field.back"),
        name: "Back".to_owned(),
        rtl: false,
        message_pattern: None,
    };
    let mut template_adapter_ids = AdapterIds::new();
    template_adapter_ids.insert("anki:ord", "0");
    let template = CardTemplate {
        id: sid("template.forward"),
        name: "Forward".to_owned(),
        variables: BTreeMap::from([("label".to_owned(), "Question".to_owned())]),
        question_format: "{{Front}}".to_owned(),
        answer_format: "{{Back}}".to_owned(),
        adapter_ids: template_adapter_ids,
    };
    let reverse_template = CardTemplate {
        id: sid("template.reverse"),
        name: "Reverse".to_owned(),
        variables: BTreeMap::new(),
        question_format: "{{Back}}".to_owned(),
        answer_format: "{{Front}}".to_owned(),
        adapter_ids: AdapterIds::new(),
    };
    let mut note_type_adapter_ids = AdapterIds::new();
    note_type_adapter_ids.insert("anki:model", "100");
    let note_type = NoteType {
        id: sid("note-type.basic"),
        name: "Basic".to_owned(),
        variables: BTreeMap::from([("label".to_owned(), "Card".to_owned())]),
        fields: vec![field.clone(), second_field],
        card_templates: vec![template.clone(), reverse_template],
        styling: ".card { color: black; }\n".to_owned(),
        adapter_ids: note_type_adapter_ids,
    };
    let mut note_adapter_ids = AdapterIds::new();
    note_adapter_ids.insert("anki:guid", "abc");
    let note = Note {
        id: sid("note.one"),
        note_type_id: sid("note-type.basic"),
        variables: BTreeMap::from([("x".to_owned(), "X".to_owned())]),
        fields: BTreeMap::from([
            (sid("field.scalar"), FieldValue::Scalar("hello".to_owned())),
            (
                sid("field.images"),
                FieldValue::Images(vec![FieldImageReference {
                    media_id: sid("media.one"),
                }]),
            ),
            (
                sid("field.message"),
                FieldValue::Message(StructuredMessage {
                    components: Vec::new(),
                    format: Some("{name}: {value}".to_owned()),
                    variables: BTreeMap::from([
                        ("name".to_owned(), MessageComponent::Text("Name".to_owned())),
                        (
                            "value".to_owned(),
                            MessageComponent::FieldRef(
                                "notes.note.one.fields.field.scalar".to_owned(),
                            ),
                        ),
                    ]),
                }),
            ),
        ])
        .into(),
        tags: BTreeSet::from(["alpha".to_owned(), "beta".to_owned()]),
        adapter_ids: note_adapter_ids,
    };
    let media = MediaReference {
        id: sid("media.one"),
        path: "images/one.png".to_owned(),
        sha256: "0123456789abcdef".to_owned(),
    };
    Fixtures {
        field,
        template,
        note_type,
        note,
        media,
    }
}

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}
