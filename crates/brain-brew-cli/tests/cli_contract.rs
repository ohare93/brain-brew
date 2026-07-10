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

fn run_with_env<I, S>(args: I, variables: &[(&str, &str)]) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(env!("CARGO_BIN_EXE_brainbrew"));
    command.args(args);
    for (name, value) in variables {
        command.env(name, value);
    }
    command.output().expect("brainbrew command runs")
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
    let error = &json["error"];
    assert_eq!(error["version"], 1, "JSON error version: {json}");
    assert!(error["code"].is_string(), "JSON error code: {json}");
    assert!(error["category"].is_string(), "JSON error category: {json}");
    assert!(error.get("source").is_some(), "JSON error source: {json}");
    assert!(error.get("path").is_some(), "JSON error path: {json}");
    assert!(error["details"].is_object(), "JSON error details: {json}");
    assert!(
        error["message"]
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

    let malformed = dir.join("malformed.yaml");
    fs::write(&malformed, "base: []\noverlays: {}\ntargets: {}\n").unwrap();
    let malformed_error = run([
        "targets",
        "--manifest",
        malformed.to_str().unwrap(),
        "--json",
    ]);
    assert_json_error(&malformed_error, "manifest YAML");
    let json: serde_json::Value = serde_json::from_slice(&malformed_error.stdout).unwrap();
    assert_eq!(json["error"]["source"], malformed.display().to_string());
    assert_eq!(json["error"]["path"], "base");
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

    let trailing_version = run(["--version", "unexpected"]);
    assert_human_error(
        &trailing_version,
        "--version does not accept trailing arguments",
    );
    let trailing_help = run(["compose", "--bogus", "--help"]);
    assert_human_error(&trailing_help, "unexpected argument");

    let bare = run(std::iter::empty::<&str>());
    assert!(bare.status.success(), "stderr: {}", stderr(&bare));
    assert!(stdout(&bare).contains("Usage:"));
    assert!(stderr(&bare).is_empty());
}

#[test]
fn compose_creates_missing_parents_and_requires_explicit_overwrite() {
    let dir = temp_dir("compose-output");
    let deck = dir.join("deck.yaml");
    let output = dir.join("nested/build/resolved.yaml");
    fs::write(&deck, SAMPLE_CANONICAL_YAML).unwrap();

    let created = run([
        "compose",
        deck.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
    ]);
    assert!(created.status.success(), "stderr: {}", stderr(&created));
    assert!(output.is_file());

    let refused = run([
        "compose",
        deck.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
    ]);
    assert_human_error(&refused, "pass --force");

    let failed_output = dir.join("ordinary/failure/resolved.yaml");
    let failed = run_with_env(
        [
            "compose",
            deck.to_str().unwrap(),
            "--out",
            failed_output.to_str().unwrap(),
        ],
        &[("BRAINBREW_TRANSACTION_FAIL_POINT", "stage:0")],
    );
    assert!(!failed.status.success());
    assert!(!failed_output.exists());
    assert!(
        !dir.join("ordinary").exists(),
        "failed compose left empty parents"
    );

    let replaced = run([
        "compose",
        deck.to_str().unwrap(),
        "--force",
        "--out",
        output.to_str().unwrap(),
    ]);
    assert!(replaced.status.success(), "stderr: {}", stderr(&replaced));
}

#[test]
fn export_force_cleanly_replaces_a_dirty_tree_and_rolls_back_publish_failure() {
    let dir = temp_dir("export-output");
    let deck = dir.join("deck.yaml");
    let output = dir.join("crowdanki");
    fs::write(&deck, SAMPLE_CANONICAL_YAML).unwrap();

    let created = run([
        "export",
        "crowdanki",
        deck.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
    ]);
    assert!(created.status.success(), "stderr: {}", stderr(&created));
    fs::write(output.join("stale.bin"), b"stale").unwrap();
    let original = fs::read(output.join("deck.json")).unwrap();

    let refused = run([
        "export",
        "crowdanki",
        deck.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
    ]);
    assert_human_error(&refused, "pass --force");
    assert!(output.join("stale.bin").exists());

    let interrupted = run_with_env(
        [
            "export",
            "crowdanki",
            deck.to_str().unwrap(),
            "--force",
            "--out",
            output.to_str().unwrap(),
        ],
        &[("BRAINBREW_OUTPUT_FAIL_POINT", "before-publish")],
    );
    assert!(!interrupted.status.success());
    assert_eq!(fs::read(output.join("deck.json")).unwrap(), original);
    assert!(output.join("stale.bin").exists());

    let crashed = run_with_env(
        [
            "export",
            "crowdanki",
            deck.to_str().unwrap(),
            "--force",
            "--out",
            output.to_str().unwrap(),
        ],
        &[
            ("BRAINBREW_OUTPUT_FAIL_POINT", "before-publish"),
            ("BRAINBREW_OUTPUT_FAIL_MODE", "crash"),
        ],
    );
    assert!(!crashed.status.success());
    assert!(
        !output.exists(),
        "uncertain output must not expose a mixed tree"
    );
    assert!(
        fs::read_dir(&dir).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("publish.json")),
        "interruption must leave explicit recovery metadata"
    );

    let replaced = run([
        "export",
        "crowdanki",
        deck.to_str().unwrap(),
        "--force",
        "--out",
        output.to_str().unwrap(),
    ]);
    assert!(replaced.status.success(), "stderr: {}", stderr(&replaced));
    assert!(!output.join("stale.bin").exists());
    assert_eq!(fs::read_dir(&output).unwrap().count(), 1);
}

#[test]
fn translations_json_failures_use_the_versioned_error_envelope() {
    let dir = temp_dir("translations-json-error");
    let missing = dir.join("missing.yaml");
    let output = run([
        "translations",
        "--manifest",
        missing.to_str().unwrap(),
        "--target",
        "missing",
        "--json",
    ]);
    assert_json_error(&output, "missing.yaml");
}

#[test]
fn diff_exit_code_distinguishes_changes_from_operational_errors() {
    let dir = temp_dir("diff-exit-code");
    let left = dir.join("left.yaml");
    let same = dir.join("same.yaml");
    let changed = dir.join("changed.yaml");
    fs::write(&left, SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(&same, SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        &changed,
        SAMPLE_CANONICAL_YAML.replace("field.capital: Helsinki", "field.capital: Turku"),
    )
    .unwrap();

    assert_eq!(
        run([
            "diff",
            left.to_str().unwrap(),
            same.to_str().unwrap(),
            "--exit-code",
        ])
        .status
        .code(),
        Some(0)
    );
    let differences = run([
        "diff",
        left.to_str().unwrap(),
        changed.to_str().unwrap(),
        "--json",
        "--exit-code",
    ]);
    assert_eq!(differences.status.code(), Some(2));
    assert!(serde_json::from_slice::<serde_json::Value>(&differences.stdout).is_ok());
    assert!(stderr(&differences).is_empty());
    assert_eq!(
        run(["diff", left.to_str().unwrap(), "--exit-code"])
            .status
            .code(),
        Some(1)
    );
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
