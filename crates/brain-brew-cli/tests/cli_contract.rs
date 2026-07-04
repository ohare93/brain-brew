use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn run<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .output()
        .expect("brainbrew command runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("brainbrew-cli-contract-{name}-{nanos}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir is created");
    dir
}

fn write_workspace(dir: &Path) {
    fs::write(dir.join("deck.yaml"), SAMPLE_CANONICAL_YAML).expect("deck fixture is written");
    fs::write(dir.join("brainbrew.yaml"), SIMPLE_MANIFEST_YAML)
        .expect("manifest fixture is written");
}

fn assert_json_error(output: &Output, expected_message: &str) {
    assert!(!output.status.success());
    assert!(stderr(output).is_empty(), "stderr: {}", stderr(output));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout is JSON");
    assert!(
        json["error"]["message"]
            .as_str()
            .expect("JSON error message is a string")
            .contains(expected_message),
        "JSON error message did not contain {expected_message:?}: {json}"
    );
}

fn assert_human_error(output: &Output, expected_stderr: &str) {
    assert!(!output.status.success());
    assert!(stdout(output).is_empty(), "stdout: {}", stdout(output));
    assert!(
        stderr(output).contains(expected_stderr),
        "stderr did not contain {expected_stderr:?}: {}",
        stderr(output)
    );
}

#[test]
fn validate_stdout_stderr_contract_covers_human_and_json_success_and_error() {
    let dir = temp_dir("validate-matrix");
    write_workspace(&dir);
    let deck = dir.join("deck.yaml");
    let invalid_overlay = dir.join("invalid-overlay.yaml");
    fs::write(&invalid_overlay, INVALID_NOTE_OVERLAY_YAML)
        .expect("invalid overlay fixture is written");

    let human_success = run(["validate", deck.to_str().unwrap()]);
    assert!(
        human_success.status.success(),
        "stderr: {}",
        stderr(&human_success)
    );
    assert!(stdout(&human_success).contains("valid deck"));
    assert!(stderr(&human_success).is_empty());

    let json_success = run(["validate", deck.to_str().unwrap(), "--json"]);
    assert!(
        json_success.status.success(),
        "stderr: {}",
        stderr(&json_success)
    );
    assert!(stderr(&json_success).is_empty());
    let json: serde_json::Value = serde_json::from_slice(&json_success.stdout).unwrap();
    assert_eq!(json["status"], "valid");

    let human_error = run([
        "validate",
        deck.to_str().unwrap(),
        "--overlay",
        invalid_overlay.to_str().unwrap(),
    ]);
    assert_human_error(&human_error, "notes.note.invalid.note_type_id");

    let json_error = run([
        "validate",
        deck.to_str().unwrap(),
        "--overlay",
        invalid_overlay.to_str().unwrap(),
        "--json",
    ]);
    assert_json_error(&json_error, "invalid deck");
    let json: serde_json::Value = serde_json::from_slice(&json_error.stdout).unwrap();
    assert_eq!(json["error"]["errors"][0]["kind"], "ValidationFailed");
    assert_eq!(
        json["error"]["errors"][0]["path"],
        "notes.note.invalid.note_type_id"
    );
}

#[test]
fn explain_stdout_stderr_contract_covers_human_and_json_success_and_error() {
    let dir = temp_dir("explain-matrix");
    write_workspace(&dir);
    let manifest = dir.join("brainbrew.yaml");

    let human_success = run([
        "explain",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "base",
    ]);
    assert!(
        human_success.status.success(),
        "stderr: {}",
        stderr(&human_success)
    );
    assert!(stdout(&human_success).contains("target: base"));
    assert!(stderr(&human_success).is_empty());

    let json_success = run([
        "explain",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "base",
        "--json",
    ]);
    assert!(
        json_success.status.success(),
        "stderr: {}",
        stderr(&json_success)
    );
    assert!(stderr(&json_success).is_empty());
    let json: serde_json::Value = serde_json::from_slice(&json_success.stdout).unwrap();
    assert_eq!(json["target"], "base");

    let human_error = run([
        "explain",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "missing",
    ]);
    assert_human_error(&human_error, "available targets: base");

    let json_error = run([
        "explain",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "missing",
        "--json",
    ]);
    let json: serde_json::Value =
        serde_json::from_slice(&json_error.stdout).expect("stdout is JSON");
    assert!(!json_error.status.success());
    assert!(
        stderr(&json_error).is_empty(),
        "stderr: {}",
        stderr(&json_error)
    );
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("available targets: base")
    );
}

#[test]
fn diff_stdout_stderr_contract_covers_human_and_json_success_and_error() {
    let dir = temp_dir("diff-matrix");
    let left = dir.join("left.yaml");
    let right = dir.join("right.yaml");
    fs::write(&left, SAMPLE_CANONICAL_YAML).expect("left deck is written");
    fs::write(
        &right,
        SAMPLE_CANONICAL_YAML.replace("field.capital: Helsinki", "field.capital: Helsingfors"),
    )
    .expect("right deck is written");

    let human_success = run(["diff", left.to_str().unwrap(), right.to_str().unwrap()]);
    assert!(
        human_success.status.success(),
        "stderr: {}",
        stderr(&human_success)
    );
    assert!(stdout(&human_success).contains("semantic change"));
    assert!(stderr(&human_success).is_empty());

    let json_success = run([
        "diff",
        left.to_str().unwrap(),
        right.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        json_success.status.success(),
        "stderr: {}",
        stderr(&json_success)
    );
    assert!(stderr(&json_success).is_empty());
    let json: serde_json::Value = serde_json::from_slice(&json_success.stdout).unwrap();
    assert_eq!(
        json["changes"][0]["path"],
        "notes.note.finland.fields.field.capital"
    );

    let human_error = run(["diff", left.to_str().unwrap()]);
    assert_human_error(&human_error, "usage: brainbrew diff");

    let json_error = run(["diff", left.to_str().unwrap(), "--json"]);
    assert_json_error(
        &json_error,
        "usage: brainbrew diff <left.yaml> <right.yaml> [--json]",
    );
}

#[test]
fn targets_stdout_stderr_contract_covers_human_and_json_success_and_error() {
    let dir = temp_dir("targets-matrix");
    write_workspace(&dir);
    let manifest = dir.join("brainbrew.yaml");
    let missing = dir.join("missing.yaml");

    let human_success = run(["targets", "--manifest", manifest.to_str().unwrap()]);
    assert!(
        human_success.status.success(),
        "stderr: {}",
        stderr(&human_success)
    );
    assert!(stdout(&human_success).contains("base"));
    assert!(stderr(&human_success).is_empty());

    let json_success = run([
        "targets",
        "--manifest",
        manifest.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        json_success.status.success(),
        "stderr: {}",
        stderr(&json_success)
    );
    assert!(stderr(&json_success).is_empty());
    let json: serde_json::Value = serde_json::from_slice(&json_success.stdout).unwrap();
    assert_eq!(json["targets"][0]["name"], "base");

    let human_error = run(["targets", "--manifest", missing.to_str().unwrap()]);
    assert_human_error(&human_error, "missing.yaml");

    let json_error = run(["targets", "--manifest", missing.to_str().unwrap(), "--json"]);
    let json: serde_json::Value =
        serde_json::from_slice(&json_error.stdout).expect("stdout is JSON");
    assert!(!json_error.status.success());
    assert!(
        stderr(&json_error).is_empty(),
        "stderr: {}",
        stderr(&json_error)
    );
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("missing.yaml")
    );
}

#[test]
fn unknown_flags_missing_flag_values_and_mutually_exclusive_flags_are_rejected() {
    let dir = temp_dir("arg-errors");
    write_workspace(&dir);
    let deck = dir.join("deck.yaml");
    let manifest = dir.join("brainbrew.yaml");

    let diff_unknown = run(["diff", "--bogus-flag", deck.to_str().unwrap()]);
    assert_human_error(&diff_unknown, "unexpected argument \"--bogus-flag\"");

    let compose_unknown = run(["compose", "--bogus-flag", deck.to_str().unwrap()]);
    assert_human_error(&compose_unknown, "unexpected argument \"--bogus-flag\"");

    let missing_explain_target_value = run([
        "explain",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
    ]);
    assert_human_error(&missing_explain_target_value, "--target requires a name");

    let missing_targets_manifest_value = run(["targets", "--manifest"]);
    assert_human_error(
        &missing_targets_manifest_value,
        "--manifest requires a path",
    );

    let mutually_exclusive_verify_flags = run([
        "verify",
        "--manifest",
        manifest.to_str().unwrap(),
        "--all-targets",
        "--target",
        "base",
    ]);
    assert_human_error(
        &mutually_exclusive_verify_flags,
        "choose --all-targets or --target, not both",
    );
}

#[test]
fn help_flags_work_at_every_position_for_multi_word_subcommands() {
    let subcommands = [
        ["lock", "update"],
        ["lock", "verify"],
        ["import", "crowdanki"],
        ["export", "crowdanki"],
    ];

    for words in subcommands {
        for flag in ["--help", "-h"] {
            for position in 0..=words.len() {
                let mut args = words.to_vec();
                args.insert(position, flag);
                let output = run(args);
                assert!(
                    output.status.success(),
                    "{flag} at position {position} for {words:?} failed: {}",
                    stderr(&output)
                );
                assert!(stdout(&output).contains("Usage:"));
                assert!(stderr(&output).is_empty(), "stderr: {}", stderr(&output));
            }
        }
    }
}

#[test]
fn version_and_bare_invocation_are_stable() {
    let version = run(["--version"]);
    assert!(version.status.success(), "stderr: {}", stderr(&version));
    assert!(stdout(&version).starts_with("brainbrew "));
    assert!(stderr(&version).is_empty());

    let bare = run(std::iter::empty::<&str>());
    assert!(bare.status.success(), "stderr: {}", stderr(&bare));
    assert!(stdout(&bare).contains("Usage:"));
    assert!(stderr(&bare).is_empty());
}

const SIMPLE_MANIFEST_YAML: &str = r#"base: deck.yaml
overlays: {}
targets:
  base:
    overlays: []
"#;

const INVALID_NOTE_OVERLAY_YAML: &str = r#"id: overlay.extension.invalid-note
kind: extension
notes:
  note.invalid:
    intent: add
    note:
      note_type_id: note-type.missing
      fields: {}
      tags: []
"#;

const SAMPLE_CANONICAL_YAML: &str = r#"deck:
  id: deck.ultimate-geography
  name: Ultimate Geography
  description: A geography deck fixture.
  adapter_ids:
    crowdanki:uuid: 43c5ba66-9a65-11e8-90c9-a0481cc15658
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.country
      - field.capital
      - field.flag
    fields:
      field.capital:
        name: Capital
      field.country:
        name: Country
      field.flag:
        name: Flag
    card_template_order:
      - template.country-capital
    card_templates:
      template.country-capital:
        name: Country - Capital
        question_format: '{{Country}}'
        answer_format: '{{FrontSide}}<hr id=answer>{{Capital}}'
        adapter_ids: {}
    styling: |
      .card { font-family: sans-serif; }
    adapter_ids:
      crowdanki:uuid: aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa
notes:
  note.finland:
    note_type_id: note-type.country
    fields:
      field.capital: Helsinki
      field.country: Finland
      field.flag: '<img src="flags/fi.png">'
    tags:
      - Europe
      - Nordic
    adapter_ids:
      crowdanki:guid: ug-finland-guid
media:
  media.flags-fi-png:
    path: flags/fi.png
    sha256: ''
tombstones: []
"#;
