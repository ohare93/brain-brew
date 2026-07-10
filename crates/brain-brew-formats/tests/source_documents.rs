use std::collections::{BTreeMap, BTreeSet};

use brain_brew_core::StableId;
use brain_brew_formats::canonical_source_document::{
    CanonicalScalarTarget, CanonicalSourceDocument,
};
use brain_brew_formats::overlay_source_document::{
    OverlaySourceDocument, SourceTranslationImpact, TranslationDecision, TranslationStubs,
};
use brain_brew_formats::source_document::{EditLocation, SourceFile, SourceProvenance};

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn source(name: &str, text: impl Into<String>) -> SourceFile {
    SourceFile::new(
        SourceProvenance::new(name).with_source_root("fixture.workspace"),
        text,
    )
}

fn deck_source() -> String {
    "deck:\n  id: deck.source-doc\n  name: Source document\n  description: !include content/description.md\n  adapter_ids: {}\nnote_types:\n  note-type.basic:\n    name: Basic\n    field_order:\n      - field.front\n      - field.image\n    fields:\n      field.front:\n        name: Front\n      field.image:\n        name: Image\n    card_template_order:\n      - template.basic\n    card_templates:\n      template.basic:\n        name: Card\n        question_format: !include templates/front.html\n        answer_format: '{{Front}}'\n        adapter_ids: {}\n    styling: ''\n    adapter_ids: {}\nnotes:\n  note.one:\n    note_type_id: note-type.basic\n    fields:\n      field.front: Before\n      field.image: '<img src=\"flag.svg\" />'\n    tags: []\n    adapter_ids: {}\nmedia: !include media.yaml\ntombstones: []\n".to_owned()
}

fn overlay_source() -> String {
    "id: overlay.translation.da\nkind: translation\ntranslations:\n  direct:\n    Before: Før\nnotes:\n  note.one:\n    intent: merge\n    fields:\n      field.front:\n        intent: replace\n        value: !include content/replacement.md\n        expected_base:\n          value: Before\n      field.image:\n        intent: replace\n        value: '<img src=\"flag.svg\" />'\n        expected_base:\n          value: ''\nmedia:\n  media.flag:\n    intent: add\n    path: flag.svg\n    sha256: old-overlay-hash\n".to_owned()
}

fn load_include(target: &str) -> SourceFile {
    match target {
        "content/description.md" => source("content/description.md", "Included description\n"),
        "templates/front.html" => source("templates/front.html", "<b>{{Front}}</b>\n"),
        "content/replacement.md" => source("content/replacement.md", "Replacement from file\n"),
        "media.yaml" => source(
            "media.yaml",
            "media.flag:\n  path: flag.svg\n  sha256: old-hash\n",
        ),
        other => panic!("unexpected include {other}"),
    }
}

#[test]
fn canonical_document_preserves_scalar_and_media_includes_through_unrelated_field_edit() {
    let mut document = CanonicalSourceDocument::parse_with_includes(
        source("deck.yaml", deck_source()),
        |request| Ok(load_include(request.target())),
    )
    .expect("include-bearing deck parses");
    assert_eq!(
        document.emit().unwrap().root().text(),
        deck_source(),
        "already-canonical root bytes are stable"
    );

    let location = document
        .set_scalar(
            CanonicalScalarTarget::NoteField {
                note_id: sid("note.one"),
                field_id: sid("field.front"),
            },
            "Before",
            "After",
        )
        .expect("field edit is validated");
    assert_eq!(location, EditLocation::Root);

    let emission = document.emit().expect("edited deck emits canonically");
    assert!(
        emission
            .root()
            .text()
            .contains("description: !include content/description.md\n")
    );
    assert!(
        emission
            .root()
            .text()
            .contains("question_format: !include templates/front.html\n")
    );
    assert!(
        emission
            .root()
            .text()
            .contains("media: !include media.yaml\n")
    );
    assert!(emission.root().text().contains("field.front: After\n"));
    assert_eq!(
        emission.root().text(),
        deck_source().replace("field.front: Before", "field.front: After"),
        "an edit to canonical source changes only its canonical field bytes"
    );
    assert!(
        emission.included().is_empty(),
        "unrelated includes stay byte-local"
    );

    let reparsed =
        CanonicalSourceDocument::parse_with_includes(emission.root().clone(), |request| {
            Ok(load_include(request.target()))
        })
        .expect("emitted source remains strict");
    assert_eq!(
        reparsed.emit().unwrap().root().text(),
        emission.root().text()
    );
}

#[test]
fn canonical_document_routes_targeted_scalar_and_media_edits_to_included_sources() {
    let mut document = CanonicalSourceDocument::parse_with_includes(
        source("deck.yaml", deck_source()),
        |request| Ok(load_include(request.target())),
    )
    .unwrap();

    let location = document
        .set_scalar(
            CanonicalScalarTarget::DeckDescription,
            "Included description\n",
            "Edited description\n",
        )
        .unwrap();
    assert_eq!(
        location,
        EditLocation::Included(
            SourceProvenance::new("content/description.md").with_source_root("fixture.workspace")
        )
    );
    let media_location = document
        .set_media_hash(&sid("media.flag"), "flag.svg", "new-hash")
        .unwrap();
    assert_eq!(
        media_location,
        EditLocation::Included(
            SourceProvenance::new("media.yaml").with_source_root("fixture.workspace")
        )
    );

    let emission = document.emit().unwrap();
    assert!(
        emission
            .root()
            .text()
            .contains("description: !include content/description.md\n")
    );
    assert!(
        emission
            .root()
            .text()
            .contains("media: !include media.yaml\n")
    );
    assert_eq!(emission.included().len(), 2);
    assert_eq!(
        emission
            .included_source("content/description.md")
            .unwrap()
            .text(),
        "Edited description\n"
    );
    assert!(
        emission
            .included_source("media.yaml")
            .unwrap()
            .text()
            .contains("sha256: new-hash\n")
    );
    assert_eq!(
        emission
            .original_source(
                &SourceProvenance::new("deck.yaml").with_source_root("fixture.workspace")
            )
            .unwrap()
            .text(),
        deck_source()
    );
    assert_eq!(
        emission
            .original_source(
                &SourceProvenance::new("content/description.md")
                    .with_source_root("fixture.workspace")
            )
            .unwrap()
            .text(),
        "Included description\n",
        "included outputs retain the exact loader bytes used at edit computation time"
    );
}

#[test]
fn overlay_document_preserves_includes_and_supports_translation_field_media_and_image_edits() {
    let mut document = OverlaySourceDocument::parse_with_includes(
        source("overlays/da.yaml", overlay_source()),
        |request| Ok(load_include(request.target())),
    )
    .expect("overlay parses");

    document
        .set_translation_decision(
            "notes.note.one.fields.field.front",
            "Before",
            TranslationDecision::Contextual {
                context: "notes.note.one".to_owned(),
                target: "Efter".to_owned(),
            },
        )
        .unwrap();
    document
        .apply_source_translation_impact(
            "notes.note.one.fields.field.front",
            "Before",
            "After source edit",
            SourceTranslationImpact::MarkStale {
                target: "Efter".to_owned(),
                context: Some("notes.note.one".to_owned()),
            },
        )
        .unwrap();
    let mut stubs = TranslationStubs::default();
    stubs.direct.insert("Card".to_owned());
    stubs.no_change.insert("Source document".to_owned());
    stubs.ignore_paths.insert("notes.*.tags.*".to_owned());
    document.add_translation_stubs(stubs).unwrap();
    document
        .set_note_field_text(
            &sid("note.one"),
            &sid("field.front"),
            "Replacement from file\n",
            "Replacement edited\n",
        )
        .unwrap();
    document
        .set_media_hash(&sid("media.flag"), "flag.svg", "new-overlay-hash")
        .unwrap();

    let lookup = BTreeMap::from([("flag.svg".to_owned(), Some(sid("media.flag")))]);
    let report = document.convert_strict_image_fields(&lookup).unwrap();
    assert_eq!(report.converted, 1);

    let emission = document.emit().unwrap();
    assert!(
        emission
            .root()
            .text()
            .contains("value: !include content/replacement.md\n")
    );
    assert!(
        emission
            .root()
            .text()
            .contains("value: !image media.flag\n")
    );
    assert!(
        emission
            .root()
            .text()
            .contains("sha256: new-overlay-hash\n")
    );
    assert!(emission.root().text().contains("stale_translations:\n"));
    assert!(
        emission
            .root()
            .text()
            .contains("new_source: After source edit\n")
    );
    assert_eq!(
        emission
            .included_source("content/replacement.md")
            .unwrap()
            .text(),
        "Replacement edited\n"
    );

    OverlaySourceDocument::parse_with_includes(emission.root().clone(), |request| {
        if request.target() == "content/replacement.md" {
            Ok(source("content/replacement.md", "Replacement edited\n"))
        } else {
            Ok(load_include(request.target()))
        }
    })
    .expect("all overlay edits emit strict source");
}

#[test]
fn overlay_translation_edits_route_included_targets_and_migrated_keys() {
    let root = source(
        "overlays/da.yaml",
        "id: overlay.translation.da\nkind: translation\ntranslations:\n  direct:\n    Before: !include content/target.txt\n",
    );
    let mut document = OverlaySourceDocument::parse_with_includes(root, |request| {
        assert_eq!(request.target(), "content/target.txt");
        Ok(source("content/target.txt", "Før"))
    })
    .unwrap();

    document
        .set_translation_decision(
            "notes.note.one.fields.field.front",
            "Before",
            TranslationDecision::Direct("Efter".to_owned()),
        )
        .unwrap();
    let emission = document.emit().unwrap();
    assert!(
        emission
            .root()
            .text()
            .contains("Before: !include content/target.txt")
    );
    assert_eq!(
        emission
            .included_source("content/target.txt")
            .unwrap()
            .text(),
        "Efter"
    );

    document
        .apply_source_translation_impact(
            "notes.note.one.fields.field.front",
            "Before",
            "After",
            SourceTranslationImpact::MigrateKey {
                target: "Efter".to_owned(),
                context: None,
            },
        )
        .unwrap();
    let emission = document.emit().unwrap();
    assert!(
        emission
            .root()
            .text()
            .contains("After: !include content/target.txt")
    );
    assert!(!emission.root().text().contains("Before:"));
}

#[test]
fn overlay_stale_resolution_removes_shadowed_record_without_overwriting_current_decision() {
    let mut document = OverlaySourceDocument::parse(source(
        "overlays/da.yaml",
        "id: overlay.translation.da\nkind: translation\ntranslations:\n  direct:\n    New source: Current target\nstale_translations:\n  - old_source: Old source\n    new_source: New source\n    target: Retained stale target\n",
    ))
    .unwrap();

    document
        .resolve_stale_translation("Old source", "New source", None, Some("replacement"))
        .unwrap();

    let output = document.emit().unwrap();
    assert!(
        output
            .root()
            .text()
            .contains("New source: Current target\n")
    );
    assert!(!output.root().text().contains("replacement"));
    assert!(!output.root().text().contains("stale_translations"));
}

#[test]
fn source_document_errors_are_source_and_schema_aware() {
    let duplicate = deck_source().replace(
        "      field.front: Before\n",
        "      field.front: Before\n      field.front: Lost\n",
    );
    let error =
        CanonicalSourceDocument::parse_with_includes(source("deck.yaml", duplicate), |request| {
            Ok(load_include(request.target()))
        })
        .expect_err("duplicates fail before mutation");
    let message = error.to_string();
    assert!(message.contains("deck.yaml"), "{message}");
    assert!(
        message.contains("notes.note.one.fields.field.front"),
        "{message}"
    );

    let non_string_scalar = deck_source().replace("name: Source document", "name: true");
    let error = CanonicalSourceDocument::parse_with_includes(
        source("deck.yaml", non_string_scalar),
        |request| Ok(load_include(request.target())),
    )
    .expect_err("non-string schema scalars fail");
    let message = error.to_string();
    assert!(message.contains("deck.yaml"), "{message}");
    assert!(message.contains("deck.name"), "{message}");

    let malformed_union = overlay_source().replace(
        "        value: !include content/replacement.md\n",
        "        value: Changed\n        message:\n          - literal: conflict\n",
    );
    let error = OverlaySourceDocument::parse(source("overlays/da.yaml", malformed_union))
        .expect_err("malformed union fails");
    let message = error.to_string();
    assert!(message.contains("overlays/da.yaml"), "{message}");
    assert!(
        message.contains("notes.note.one.fields.field.front"),
        "{message}"
    );

    let duplicate_media = source(
        "media.yaml",
        "media.flag:\n  path: one.svg\n  sha256: one\nmedia.flag:\n  path: two.svg\n  sha256: two\n",
    );
    let error = CanonicalSourceDocument::parse_with_includes(
        source("deck.yaml", deck_source()),
        |request| {
            if request.target() == "media.yaml" {
                Ok(duplicate_media.clone())
            } else {
                Ok(load_include(request.target()))
            }
        },
    )
    .expect_err("included media duplicates fail");
    let message = error.to_string();
    assert!(message.contains("media.yaml"), "{message}");
    assert!(message.contains("media.flag"), "{message}");

    let mut document = CanonicalSourceDocument::parse_with_includes(
        source("deck.yaml", deck_source()),
        |request| Ok(load_include(request.target())),
    )
    .unwrap();
    let error = document
        .set_scalar(
            CanonicalScalarTarget::NoteField {
                note_id: sid("note.one"),
                field_id: sid("field.front"),
            },
            "stale expected value",
            "After",
        )
        .expect_err("compare-and-set is mandatory");
    let message = error.to_string();
    assert!(message.contains("deck.yaml"), "{message}");
    assert!(
        message.contains("notes.note.one.fields.field.front"),
        "{message}"
    );
}

#[test]
fn canonical_document_converts_images_and_canonicalizes_root_but_not_untouched_includes() {
    let mut document = CanonicalSourceDocument::parse_with_includes(
        source("deck.yaml", deck_source()),
        |request| Ok(load_include(request.target())),
    )
    .unwrap();
    let lookup = BTreeMap::from([("flag.svg".to_owned(), Some(sid("media.flag")))]);
    let report = document.convert_strict_image_fields(&lookup).unwrap();
    assert_eq!(report.converted, 1);
    assert_eq!(report.skipped_ambiguous_path, 0);

    let emission = document.emit().unwrap();
    assert!(
        emission
            .root()
            .text()
            .contains("field.image: !image media.flag\n")
    );
    assert!(emission.included().is_empty());

    let mut direct = BTreeSet::new();
    direct.insert("new source".to_owned());
    let mut overlay = OverlaySourceDocument::parse(source(
        "overlay.yaml",
        "id: overlay.empty\nkind: translation\n",
    ))
    .unwrap();
    overlay
        .add_translation_stubs(TranslationStubs {
            direct,
            ..Default::default()
        })
        .unwrap();
    assert!(
        overlay
            .emit()
            .unwrap()
            .root()
            .text()
            .contains("new source: new source\n")
    );
}
