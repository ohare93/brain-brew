use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use brain_brew_formats::source_includes;

#[test]
fn bare_manifest_commands_resolve_media_and_file_includes_from_package_cwd() {
    let dir = temp_dir("bare-manifest-media-includes");
    write_media_include_workspace(&dir, "");

    let hash = run_in_dir(
        [
            "media",
            "hash",
            "--manifest",
            "brainbrew.yaml",
            "--all-targets",
            "--media-root",
            "media",
        ],
        &dir,
    );
    assert!(hash.status.success(), "stderr: {}", stderr(&hash));
    assert!(stdout(&hash).contains("files changed: 1"));
    assert!(stdout(&hash).contains("entries changed: 1"));
    assert!(
        fs::read_to_string(dir.join("media.yaml"))
            .unwrap()
            .contains("sha256: 14873f4faae48052921f9272d948a369f775b2406e57a9b8d55fb94452b73948")
    );

    let verify = run_in_dir(
        [
            "verify",
            "--manifest",
            "brainbrew.yaml",
            "--all-targets",
            "--media-root",
            "media",
        ],
        &dir,
    );
    assert!(verify.status.success(), "stderr: {}", stderr(&verify));

    let images_to_refs = run_in_dir(
        [
            "media",
            "images-to-refs",
            "--manifest",
            "brainbrew.yaml",
            "--all-targets",
        ],
        &dir,
    );
    assert!(
        images_to_refs.status.success(),
        "stderr: {}",
        stderr(&images_to_refs)
    );
    assert!(stdout(&images_to_refs).contains("converted fields: 1"));
    let deck = fs::read_to_string(dir.join("deck.yaml")).unwrap();
    assert!(
        deck.contains("description: !include content/description.md\n"),
        "{deck}"
    );
    assert!(
        deck.contains("field.flag: !image media.flags-fi-png\n"),
        "{deck}"
    );
    assert!(deck.contains("media: !include media.yaml\n"), "{deck}");
}

#[test]
fn include_resolver_package_root_failure_names_offending_path() {
    let dir = temp_dir("include-root-diagnostic");
    let missing_root = dir.join("missing-package-root");
    let source_path = missing_root.join("deck.yaml");

    let error = source_includes::resolve_file_includes(
        "deck:\n  description: !include content/description.md\n",
        &source_path,
        &missing_root,
        &[],
    )
    .expect_err("missing package root should fail");
    let message = error.to_string();

    assert!(message.contains("package root"), "{message}");
    assert!(
        message.contains(&missing_root.display().to_string()),
        "{message}"
    );
    assert!(!message.contains("include path  ()"), "{message}");
}

fn write_media_include_workspace(dir: &Path, sha256: &str) {
    fs::create_dir_all(dir.join("content")).unwrap();
    fs::create_dir_all(dir.join("media/flags")).unwrap();
    fs::write(dir.join("content/description.md"), "Included description\n").unwrap();
    fs::write(dir.join("media/flags/fi.png"), b"flag-bytes").unwrap();
    fs::write(dir.join("brainbrew.yaml"), SIMPLE_MEDIA_MANIFEST_YAML).unwrap();
    fs::write(dir.join("media.yaml"), media_map_yaml(sha256)).unwrap();
    fs::write(dir.join("deck.yaml"), hoisted_media_deck_yaml()).unwrap();
}

fn hoisted_media_deck_yaml() -> String {
    r#"deck:
  id: deck.media-fixture
  name: Media Fixture
  description: !include content/description.md
  adapter_ids:
    crowdanki:uuid: media-deck-uuid
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.country
      - field.flag
    fields:
      field.country:
        name: Country
      field.flag:
        name: Flag
    card_template_order:
      - template.country-flag
    card_templates:
      template.country-flag:
        name: Country - Flag
        question_format: '{{Country}}'
        answer_format: '{{FrontSide}}<hr id=answer>{{Flag}}'
        adapter_ids: {}
    styling: |
      .card { font-family: sans-serif; }
    adapter_ids:
      crowdanki:uuid: media-note-type-uuid
notes:
  note.finland:
    note_type_id: note-type.country
    fields:
      field.country: Finland
      field.flag: '<img src="flags/fi.png" />'
    tags:
      - Media
    adapter_ids:
      crowdanki:guid: media-fi-guid
media: !include media.yaml
tombstones: []
"#
    .to_owned()
}

fn media_map_yaml(sha256: &str) -> String {
    format!("media.flags-fi-png:\n  path: flags/fi.png\n  sha256: {sha256}\n")
}

fn run_in_dir<const N: usize>(args: [&str; N], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("command runs")
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
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

const SIMPLE_MEDIA_MANIFEST_YAML: &str = r#"base: deck.yaml
overlays: {}
targets:
  base:
    overlays: []
"#;
