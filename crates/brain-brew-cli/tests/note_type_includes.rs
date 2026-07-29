use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use brain_brew_formats::canonical_yaml;

#[test]
fn compose_validate_and_export_match_inline_note_types() {
    let dir = temp_dir("note-type-include-commands");
    fs::create_dir_all(dir.join("schema")).unwrap();
    fs::create_dir_all(dir.join("templates")).unwrap();
    fs::create_dir_all(dir.join("styles")).unwrap();
    fs::write(dir.join("brainbrew.yaml"), MANIFEST).unwrap();
    fs::write(dir.join("schema/note-types.yaml"), NOTE_TYPES).unwrap();
    fs::write(dir.join("templates/question.html"), "<b>{{Front}}</b>").unwrap();
    fs::write(
        dir.join("templates/answer.html"),
        "{{FrontSide}}<hr id=answer>{{Back}}",
    )
    .unwrap();
    fs::write(dir.join("styles/card.css"), ".card { color: navy; }").unwrap();
    fs::write(dir.join("deck.yaml"), INCLUDED_DECK).unwrap();

    let validate = run_in_dir(
        [
            "validate",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "base",
        ],
        &dir,
    );
    assert!(validate.status.success(), "stderr: {}", stderr(&validate));

    let verify = run_in_dir(
        [
            "verify",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "base",
            "--media-mode",
            "reference-only",
        ],
        &dir,
    );
    assert!(verify.status.success(), "stderr: {}", stderr(&verify));

    let compose = run_in_dir(
        [
            "compose",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "base",
            "--out",
            "resolved.yaml",
        ],
        &dir,
    );
    assert!(compose.status.success(), "stderr: {}", stderr(&compose));
    let resolved = fs::read_to_string(dir.join("resolved.yaml")).unwrap();
    let resolved_deck = canonical_yaml::from_str(&resolved).unwrap();
    let inline_deck = canonical_yaml::from_str(INLINE_DECK).unwrap();
    assert_eq!(resolved_deck, inline_deck);
    assert!(
        resolved_deck.note_types.values().next().unwrap().fields[0]
            .message_pattern
            .is_some(),
        "field message_pattern survives compose"
    );

    let export = run_in_dir(
        [
            "export",
            "crowdanki",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "base",
            "--media-mode",
            "reference-only",
            "--out",
            "export",
        ],
        &dir,
    );
    assert!(export.status.success(), "stderr: {}", stderr(&export));
    let crowdanki: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.join("export/deck.json")).unwrap()).unwrap();
    assert_eq!(crowdanki["note_models"].as_array().unwrap().len(), 1);
    assert_eq!(crowdanki["notes"].as_array().unwrap().len(), 1);

    fs::write(dir.join("deck.yaml"), INLINE_DECK).unwrap();
    let inline_export = run_in_dir(
        [
            "export",
            "crowdanki",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "base",
            "--media-mode",
            "reference-only",
            "--out",
            "inline-export",
        ],
        &dir,
    );
    assert!(
        inline_export.status.success(),
        "stderr: {}",
        stderr(&inline_export)
    );
    let inline_crowdanki: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.join("inline-export/deck.json")).unwrap())
            .unwrap();
    assert_eq!(
        crowdanki, inline_crowdanki,
        "structural note-type and inline authoring export identically"
    );
}

#[test]
fn verify_uses_included_note_type_order_with_nested_scalar_includes() {
    let dir = temp_dir("note-type-include-verify-materialized");
    fs::create_dir_all(dir.join("schema")).unwrap();
    fs::create_dir_all(dir.join("templates")).unwrap();
    fs::create_dir_all(dir.join("styles")).unwrap();
    fs::write(dir.join("brainbrew.yaml"), MANIFEST).unwrap();
    fs::write(dir.join("schema/note-types.yaml"), NOTE_TYPES).unwrap();
    fs::write(dir.join("templates/question.html"), "<b>{{Front}}</b>").unwrap();
    fs::write(
        dir.join("templates/answer.html"),
        "{{FrontSide}}<hr id=answer>{{Back}}",
    )
    .unwrap();
    fs::write(dir.join("styles/card.css"), ".card { color: navy; }").unwrap();
    let canonical_root = INCLUDED_DECK;
    fs::write(dir.join("deck.yaml"), canonical_root).unwrap();

    let verify = run_in_dir(
        [
            "verify",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "base",
            "--media-mode",
            "reference-only",
        ],
        &dir,
    );
    assert!(verify.status.success(), "stderr: {}", stderr(&verify));
    assert_eq!(
        fs::read_to_string(dir.join("deck.yaml")).unwrap(),
        canonical_root
    );
    assert!(canonical_root.contains("note_types: !include schema/note-types.yaml\n"));
}

#[test]
fn nested_note_type_scalar_includes_remain_package_root_safe() {
    let dir = temp_dir("note-type-nested-include-safety");
    fs::create_dir_all(dir.join("schema")).unwrap();
    fs::create_dir_all(dir.join("templates")).unwrap();
    fs::create_dir_all(dir.join("styles")).unwrap();
    fs::write(dir.join("brainbrew.yaml"), MANIFEST).unwrap();
    fs::write(dir.join("deck.yaml"), INCLUDED_DECK).unwrap();
    fs::write(
        dir.join("schema/note-types.yaml"),
        NOTE_TYPES.replace("templates/question.html", "../outside.html"),
    )
    .unwrap();
    fs::write(
        dir.join("templates/answer.html"),
        "{{FrontSide}}<hr id=answer>{{Back}}",
    )
    .unwrap();
    fs::write(dir.join("styles/card.css"), ".card { color: navy; }").unwrap();

    let validate = run_in_dir(
        [
            "validate",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "base",
        ],
        &dir,
    );
    assert!(!validate.status.success());
    let message = stderr(&validate);
    assert!(message.contains("schema/note-types.yaml"), "{message}");
    assert!(message.contains("question_format"), "{message}");
    assert!(message.contains("parent-directory"), "{message}");
}

#[test]
fn fmt_canonicalizes_standalone_note_type_map_and_preserves_deck_marker() {
    let dir = temp_dir("note-type-include-fmt");
    fs::create_dir_all(dir.join("schema")).unwrap();
    fs::create_dir_all(dir.join("templates")).unwrap();
    fs::create_dir_all(dir.join("styles")).unwrap();
    fs::write(dir.join("brainbrew.yaml"), MANIFEST).unwrap();
    fs::write(dir.join("templates/question.html"), "<b>{{Front}}</b>").unwrap();
    fs::write(
        dir.join("templates/answer.html"),
        "{{FrontSide}}<hr id=answer>{{Back}}",
    )
    .unwrap();
    fs::write(dir.join("styles/card.css"), ".card { color: navy; }").unwrap();
    fs::write(dir.join("deck.yaml"), INCLUDED_DECK).unwrap();
    fs::write(
        dir.join("schema/note-types.yaml"),
        NOTE_TYPES.replace("name: Basic", "name: 'Basic'"),
    )
    .unwrap();

    let map_path = dir.join("schema/note-types.yaml");
    let map_fmt = run_in_dir(["fmt", map_path.to_str().unwrap()], &dir);
    assert!(map_fmt.status.success(), "stderr: {}", stderr(&map_fmt));
    assert_eq!(
        fs::read_to_string(dir.join("schema/note-types.yaml")).unwrap(),
        NOTE_TYPES
    );

    let deck_path = dir.join("deck.yaml");
    let deck_fmt = run_in_dir(["fmt", deck_path.to_str().unwrap()], &dir);
    assert!(deck_fmt.status.success(), "stderr: {}", stderr(&deck_fmt));
    assert!(
        fs::read_to_string(dir.join("deck.yaml"))
            .unwrap()
            .contains("note_types: !include schema/note-types.yaml\n")
    );
}

fn run_in_dir<const N: usize>(args: [&str; N], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("command runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn temp_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-{unique}"));
    fs::create_dir_all(&path).unwrap();
    path
}

const MANIFEST: &str = "base: deck.yaml\noverlays: {}\ntargets:\n  base:\n    overlays: []\n";

const INCLUDED_DECK: &str = r#"deck:
  id: deck.note-types-include
  name: Note Types Include
  description: ''
  adapter_ids:
    crowdanki:uuid: deck-note-types-include
note_types: !include schema/note-types.yaml
notes:
  note.one:
    note_type_id: note-type.basic
    fields:
      field.front: Front
      field.back: Back
    tags: []
    adapter_ids:
      crowdanki:guid: note-one
media: {}
tombstones: []
"#;

const INLINE_DECK: &str = r#"deck:
  id: deck.note-types-include
  name: Note Types Include
  description: ''
  adapter_ids:
    crowdanki:uuid: deck-note-types-include
note_types:
  note-type.basic:
    name: Basic
    field_order:
      - field.front
      - field.back
    fields:
      field.front:
        name: Front
        message_pattern:
          kind: list
          item_format: '{item}'
          separator: ', '
          parameters:
            item:
              type: text
      field.back:
        name: Back
    card_template_order:
      - template.basic
    card_templates:
      template.basic:
        name: Basic
        question_format: '<b>{{Front}}</b>'
        answer_format: '{{FrontSide}}<hr id=answer>{{Back}}'
        adapter_ids: {}
    styling: '.card { color: navy; }'
    adapter_ids:
      crowdanki:uuid: note-type-basic
notes:
  note.one:
    note_type_id: note-type.basic
    fields:
      field.front: Front
      field.back: Back
    tags: []
    adapter_ids:
      crowdanki:guid: note-one
media: {}
tombstones: []
"#;

const NOTE_TYPES: &str = r#"note-type.basic:
  name: Basic
  field_order:
    - field.front
    - field.back
  fields:
    field.front:
      name: Front
      message_pattern:
        kind: list
        item_format: '{item}'
        separator: ', '
        parameters:
          item:
            type: text
    field.back:
      name: Back
  card_template_order:
    - template.basic
  card_templates:
    template.basic:
      name: Basic
      question_format: !include templates/question.html
      answer_format: !include templates/answer.html
      adapter_ids: {}
  styling: !include styles/card.css
  adapter_ids:
    crowdanki:uuid: note-type-basic
"#;
