use std::collections::BTreeMap;

use brain_brew_core::{
    AdapterIds, CanonicalDeck, CardTemplate, ContentKind, FieldDefinition, MediaReference,
    NoteType, StableId, Tombstones, validate_content_str, validate_deck_content,
};

#[derive(Clone, Copy)]
struct ParityCase {
    name: &'static str,
    kind: ContentKind,
    source: &'static str,
    python_accepted: bool,
    brain_brew_accepted: bool,
}

#[test]
fn ug_side_script_tolerance_corpus_is_table_driven() {
    // Captured before the Rust implementation by importing and running
    // /home/jmo/Development/external/ultimate-geography/scripts/check-source-content.py
    // against UG's real source tree (22 files accepted) and these deliberate variants.
    // The UG script's HTMLParser use accepts malformed tag structure; Brain Brew
    // intentionally tightens HTML tag balance while preserving the CSS behavior that
    // the Python script actually enforced.
    let cases = [
        ParityCase {
            name: "html_valid_fragment_with_anki_mustache_voids_and_entities",
            kind: ContentKind::HtmlFragment,
            source: r#"<div><span>{{Field}}</span><br><img src="flag.png">{{cloze:Text}}&amp;</div>"#,
            python_accepted: true,
            brain_brew_accepted: true,
        },
        ParityCase {
            name: "html_mismatched_tag",
            kind: ContentKind::HtmlFragment,
            source: "<div><span>bad</div>",
            python_accepted: true,
            brain_brew_accepted: false,
        },
        ParityCase {
            name: "html_unclosed_tag",
            kind: ContentKind::HtmlFragment,
            source: "<div><span>bad",
            python_accepted: true,
            brain_brew_accepted: false,
        },
        ParityCase {
            name: "html_stray_closing_tag",
            kind: ContentKind::HtmlFragment,
            source: "</div>",
            python_accepted: true,
            brain_brew_accepted: false,
        },
        ParityCase {
            name: "html_entities_are_not_semantically_validated",
            kind: ContentKind::HtmlFragment,
            source: "&notareal; &unterminated",
            python_accepted: true,
            brain_brew_accepted: true,
        },
        ParityCase {
            name: "css_valid_ignores_braces_inside_strings_and_comments",
            kind: ContentKind::Css,
            source: r#".card { content: "}"; /* { */ color: red; }"#,
            python_accepted: true,
            brain_brew_accepted: true,
        },
        ParityCase {
            name: "css_unbalanced_brace",
            kind: ContentKind::Css,
            source: ".card { color: red;",
            python_accepted: false,
            brain_brew_accepted: false,
        },
        ParityCase {
            name: "css_stray_closer",
            kind: ContentKind::Css,
            source: ".card }",
            python_accepted: false,
            brain_brew_accepted: false,
        },
        ParityCase {
            name: "css_unterminated_comment",
            kind: ContentKind::Css,
            source: ".card { /* nope",
            python_accepted: false,
            brain_brew_accepted: false,
        },
        ParityCase {
            name: "css_unterminated_string",
            kind: ContentKind::Css,
            source: ".card { content: \"nope; }",
            python_accepted: false,
            brain_brew_accepted: false,
        },
        ParityCase {
            name: "css_balanced_parens_and_brackets",
            kind: ContentKind::Css,
            source: ".card { color: rgb(1, 2, 3); grid-template: [x]; }",
            python_accepted: true,
            brain_brew_accepted: true,
        },
        ParityCase {
            name: "css_mismatched_pair",
            kind: ContentKind::Css,
            source: ".card { color: rgb(1, 2, 3]; }",
            python_accepted: false,
            brain_brew_accepted: false,
        },
    ];

    assert!(
        cases
            .iter()
            .any(|case| case.python_accepted != case.brain_brew_accepted),
        "the corpus should preserve the observed Python/Brain Brew HTML tolerance delta"
    );
    for case in cases {
        let report = validate_content_str(case.kind, case.name, case.source);
        assert_eq!(
            report.is_empty(),
            case.brain_brew_accepted,
            "{} produced {report}",
            case.name
        );
    }
}

#[test]
fn deck_content_report_points_at_description_templates_and_styling() {
    let report = validate_deck_content(&deck_with_content(
        "<p>ok</p>",
        "<section>{{Country}}",
        "{{FrontSide}}<hr id=answer><strong>{{Capital}}</strong>",
        ".card { color: red;",
    ));

    assert_eq!(report.errors.len(), 2, "{report}");
    let rendered = report.to_string();
    assert!(rendered.contains(
        "note_types.note-type.country.card_templates.template.country-capital.question_format:1: unclosed HTML tag <section>"
    ));
    assert!(rendered.contains("note_types.note-type.country.styling:1: unmatched '{'"));
}

#[test]
fn deck_content_accepts_valid_anki_fragments_and_css() {
    let report = validate_deck_content(&deck_with_content(
        "<p>Geography &amp; maps</p>",
        r#"{{#Country}}<img src="{{Flag}}"><br>{{cloze:Country}}{{/Country}}"#,
        "{{FrontSide}}<hr id=answer><div>{{Capital}}</div>",
        r#".card { font-family: "Noto Sans"; } .flag::after { content: "}"; }"#,
    ));

    assert!(report.is_empty(), "{report}");
}

fn deck_with_content(
    description: &str,
    question_format: &str,
    answer_format: &str,
    styling: &str,
) -> CanonicalDeck {
    let note_type_id = stable_id("note-type.country");
    let template_id = stable_id("template.country-capital");
    let mut note_types = BTreeMap::new();
    note_types.insert(
        note_type_id.clone(),
        NoteType {
            id: note_type_id,
            name: "Country".to_owned(),
            variables: BTreeMap::new(),
            fields: vec![FieldDefinition {
                id: stable_id("field.country"),
                name: "Country".to_owned(),
                rtl: false,
                message_pattern: None,
            }],
            card_templates: vec![CardTemplate {
                id: template_id,
                name: "Country - Capital".to_owned(),
                variables: BTreeMap::new(),
                question_format: question_format.to_owned(),
                answer_format: answer_format.to_owned(),
                adapter_ids: AdapterIds::new(),
            }],
            styling: styling.to_owned(),
            adapter_ids: AdapterIds::new(),
        },
    );

    CanonicalDeck {
        id: stable_id("deck.content-validation"),
        name: "Content Validation".to_owned(),
        description: description.to_owned(),
        variables: BTreeMap::new(),
        note_types,
        notes: BTreeMap::new(),
        media: BTreeMap::<StableId, MediaReference>::new(),
        tombstones: Tombstones::default(),
        adapter_ids: AdapterIds::new(),
    }
}

fn stable_id(value: &str) -> StableId {
    StableId::new(value).unwrap()
}
