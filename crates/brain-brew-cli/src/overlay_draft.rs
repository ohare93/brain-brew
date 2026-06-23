use std::collections::BTreeMap;

use brain_brew_core::{
    AdapterIdChange, AdapterIds, CanonicalDeck, CardTemplateChange, ChangeIntent, DeckChange,
    ExpectedBase, FieldChange, FieldDefinitionChange, MediaChange, MediaReference, NoteChange,
    NoteTypeChange, Overlay, OverlayKind, PropertyChange, StableId, TagChange,
};

pub(crate) fn draft_overlay_from_diff(
    left: &CanonicalDeck,
    right: &CanonicalDeck,
    id: StableId,
    kind: OverlayKind,
) -> Result<Overlay, String> {
    let mut overlay = Overlay {
        id,
        kind,
        translations: None,
        deck_change: None,
        note_changes: BTreeMap::new(),
        note_type_changes: BTreeMap::new(),
        media_changes: BTreeMap::new(),
    };

    draft_deck_changes(left, right, &mut overlay);
    draft_note_type_adapter_changes(left, right, &mut overlay);
    draft_note_changes(left, right, &mut overlay);
    draft_media_changes(left, right, &mut overlay);

    if overlay.deck_change.is_none()
        && overlay.note_changes.is_empty()
        && overlay.note_type_changes.is_empty()
        && overlay.media_changes.is_empty()
        && !left.semantic_diff(right).is_empty()
    {
        return Err(
            "diff --as-overlay currently supports deck name/description, adapter IDs, tags, media references, note additions/removals, and existing note field changes"
                .to_owned(),
        );
    }

    Ok(overlay)
}

fn draft_deck_changes(left: &CanonicalDeck, right: &CanonicalDeck, overlay: &mut Overlay) {
    let mut deck_change = DeckChange {
        name: None,
        description: None,
        variables: BTreeMap::new(),
        adapter_ids: adapter_id_changes(&left.adapter_ids, &right.adapter_ids),
    };
    if left.name != right.name {
        deck_change.name = Some(replace_property_change(&left.name, &right.name));
    }
    if left.description != right.description {
        deck_change.description = Some(replace_property_change(
            &left.description,
            &right.description,
        ));
    }
    if deck_change.name.is_some()
        || deck_change.description.is_some()
        || !deck_change.adapter_ids.is_empty()
    {
        overlay.deck_change = Some(deck_change);
    }
}

fn draft_note_type_adapter_changes(
    left: &CanonicalDeck,
    right: &CanonicalDeck,
    overlay: &mut Overlay,
) {
    for (note_type_id, left_note_type) in &left.note_types {
        let Some(right_note_type) = right.note_types.get(note_type_id) else {
            continue;
        };
        let adapter_ids =
            adapter_id_changes(&left_note_type.adapter_ids, &right_note_type.adapter_ids);
        if !adapter_ids.is_empty() {
            overlay.note_type_changes.insert(
                note_type_id.clone(),
                NoteTypeChange {
                    intent: ChangeIntent::Merge,
                    note_type: None,
                    name: None,
                    variables: BTreeMap::new(),
                    styling: None,
                    fields: BTreeMap::<StableId, FieldDefinitionChange>::new(),
                    card_templates: BTreeMap::<StableId, CardTemplateChange>::new(),
                    adapter_ids,
                    expected_base: None,
                },
            );
        }
    }
}

fn draft_note_changes(left: &CanonicalDeck, right: &CanonicalDeck, overlay: &mut Overlay) {
    for (note_id, left_note) in &left.notes {
        if !right.notes.contains_key(note_id) {
            overlay.note_changes.insert(
                note_id.clone(),
                NoteChange {
                    intent: ChangeIntent::Remove,
                    note: None,
                    variables: BTreeMap::new(),
                    fields: BTreeMap::new(),
                    tags: BTreeMap::new(),
                    adapter_ids: BTreeMap::new(),
                    expected_base: Some(ExpectedBase::EntityPresent),
                },
            );
            continue;
        }

        let right_note = &right.notes[note_id];
        let mut fields = BTreeMap::new();
        for (field_id, left_value) in &left_note.fields {
            let Some(right_value) = right_note.fields.get(field_id) else {
                continue;
            };
            if left_value != right_value {
                fields.insert(
                    field_id.clone(),
                    FieldChange {
                        intent: ChangeIntent::Replace,
                        value: Some(right_value.clone()),
                        message: None,
                        expected_base: Some(ExpectedBase::Value(left_value.clone())),
                    },
                );
            }
        }

        let mut tags = BTreeMap::new();
        for tag in left_note.tags.difference(&right_note.tags) {
            tags.insert(
                tag.clone(),
                TagChange {
                    intent: ChangeIntent::Remove,
                    expected_base: Some(ExpectedBase::EntityPresent),
                },
            );
        }
        for tag in right_note.tags.difference(&left_note.tags) {
            tags.insert(
                tag.clone(),
                TagChange {
                    intent: ChangeIntent::Add,
                    expected_base: None,
                },
            );
        }

        let adapter_ids = adapter_id_changes(&left_note.adapter_ids, &right_note.adapter_ids);

        if !fields.is_empty() || !tags.is_empty() || !adapter_ids.is_empty() {
            overlay.note_changes.insert(
                note_id.clone(),
                NoteChange {
                    intent: ChangeIntent::Merge,
                    note: None,
                    variables: BTreeMap::new(),
                    fields,
                    tags,
                    adapter_ids,
                    expected_base: None,
                },
            );
        }
    }

    for (note_id, right_note) in &right.notes {
        if !left.notes.contains_key(note_id) {
            overlay.note_changes.insert(
                note_id.clone(),
                NoteChange {
                    intent: ChangeIntent::Add,
                    note: Some(right_note.clone()),
                    variables: BTreeMap::new(),
                    fields: BTreeMap::new(),
                    tags: BTreeMap::new(),
                    adapter_ids: BTreeMap::new(),
                    expected_base: None,
                },
            );
        }
    }
}

fn draft_media_changes(left: &CanonicalDeck, right: &CanonicalDeck, overlay: &mut Overlay) {
    for (media_id, left_media) in &left.media {
        match right.media.get(media_id) {
            Some(right_media) if left_media != right_media => {
                overlay.media_changes.insert(
                    media_id.clone(),
                    MediaChange {
                        intent: ChangeIntent::Replace,
                        media: Some(right_media.clone()),
                        expected_base: Some(ExpectedBase::Value(media_reference_summary(
                            left_media,
                        ))),
                    },
                );
            }
            Some(_) => {}
            None => {
                overlay.media_changes.insert(
                    media_id.clone(),
                    MediaChange {
                        intent: ChangeIntent::Remove,
                        media: None,
                        expected_base: Some(ExpectedBase::Value(media_reference_summary(
                            left_media,
                        ))),
                    },
                );
            }
        }
    }

    for (media_id, right_media) in &right.media {
        if !left.media.contains_key(media_id) {
            overlay.media_changes.insert(
                media_id.clone(),
                MediaChange {
                    intent: ChangeIntent::Add,
                    media: Some(right_media.clone()),
                    expected_base: None,
                },
            );
        }
    }
}

fn adapter_id_changes(left: &AdapterIds, right: &AdapterIds) -> BTreeMap<String, AdapterIdChange> {
    let left = left
        .iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect::<BTreeMap<_, _>>();
    let right = right
        .iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect::<BTreeMap<_, _>>();
    let mut changes = BTreeMap::new();

    for (key, left_value) in &left {
        match right.get(key) {
            Some(right_value) if left_value != right_value => {
                changes.insert(
                    key.clone(),
                    AdapterIdChange {
                        intent: ChangeIntent::Replace,
                        value: Some(right_value.clone()),
                        expected_base: Some(ExpectedBase::Value(left_value.clone())),
                    },
                );
            }
            Some(_) => {}
            None => {
                changes.insert(
                    key.clone(),
                    AdapterIdChange {
                        intent: ChangeIntent::Remove,
                        value: None,
                        expected_base: Some(ExpectedBase::Value(left_value.clone())),
                    },
                );
            }
        }
    }

    for (key, right_value) in &right {
        if !left.contains_key(key) {
            changes.insert(
                key.clone(),
                AdapterIdChange {
                    intent: ChangeIntent::Add,
                    value: Some(right_value.clone()),
                    expected_base: None,
                },
            );
        }
    }

    changes
}

fn media_reference_summary(media: &MediaReference) -> String {
    format!("path={};sha256={}", media.path, media.sha256)
}

fn replace_property_change(before: &str, after: &str) -> PropertyChange {
    PropertyChange {
        intent: ChangeIntent::Replace,
        value: Some(after.to_owned()),
        expected_base: Some(ExpectedBase::Value(before.to_owned())),
    }
}
