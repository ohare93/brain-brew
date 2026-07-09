use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use brain_brew_formats::canonical_yaml;

#[test]
fn resolve_confirm_and_replace_promote_current_stale_records() {
    let confirm_dir = temp_dir("translations-confirm-promote");
    write_workspace(&confirm_dir, "Helsinki City", STALE_ONLY_OVERLAY);
    let confirm = run([
        "translations",
        "--manifest",
        confirm_dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--resolve",
        "confirm",
        "--old-source",
        "Helsinki",
        "--new-source",
        "Helsinki City",
    ]);
    assert!(confirm.status.success(), "stderr: {}", stderr(&confirm));
    assert!(
        fs::read_to_string(confirm_dir.join("da.yaml"))
            .unwrap()
            .contains("Helsinki City: Helsingfors")
    );

    let replace_dir = temp_dir("translations-replace-promote");
    write_workspace(&replace_dir, "Helsinki City", STALE_ONLY_OVERLAY);
    let replace = run([
        "translations",
        "--manifest",
        replace_dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--resolve",
        "replace",
        "--old-source",
        "Helsinki",
        "--new-source",
        "Helsinki City",
        "--translation",
        "Helsingfors stad",
    ]);
    assert!(replace.status.success(), "stderr: {}", stderr(&replace));
    assert!(
        fs::read_to_string(replace_dir.join("da.yaml"))
            .unwrap()
            .contains("Helsinki City: Helsingfors stad")
    );
}

#[test]
fn resolve_deletes_shadowed_stale_record_and_clears_strict_verify() {
    let dir = temp_dir("translations-shadowed-delete");
    write_workspace(&dir, "Helsinki City", SHADOWED_STALE_OVERLAY);

    let strict_before = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-release",
    ]);
    assert!(!strict_before.status.success());
    assert!(
        command_text(&strict_before).contains("translation stale strict policy failed"),
        "stdout: {}\nstderr: {}",
        stdout(&strict_before),
        stderr(&strict_before)
    );

    let resolved = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--resolve",
        "confirm",
        "--old-source",
        "Helsinki",
        "--new-source",
        "Helsinki City",
    ]);
    assert!(resolved.status.success(), "stderr: {}", stderr(&resolved));
    let overlay = fs::read_to_string(dir.join("da.yaml")).unwrap();
    assert!(overlay.contains("Helsinki City: Helsingfors by"));
    assert!(!overlay.contains("stale_translations"));

    let strict_after = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-release",
    ]);
    assert!(
        strict_after.status.success(),
        "stderr: {}",
        stderr(&strict_after)
    );

    let replace_dir = temp_dir("translations-shadowed-delete-replace");
    write_workspace(&replace_dir, "Helsinki City", SHADOWED_STALE_OVERLAY);
    let replace = run([
        "translations",
        "--manifest",
        replace_dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--resolve",
        "replace",
        "--old-source",
        "Helsinki",
        "--new-source",
        "Helsinki City",
        "--translation",
        "ignored replacement for shadowed record",
    ]);
    assert!(replace.status.success(), "stderr: {}", stderr(&replace));
    let replace_overlay = fs::read_to_string(replace_dir.join("da.yaml")).unwrap();
    assert!(replace_overlay.contains("Helsinki City: Helsingfors by"));
    assert!(!replace_overlay.contains("stale_translations"));
    assert!(!replace_overlay.contains("ignored replacement for shadowed record"));
}

#[test]
fn translations_argument_conflicts_are_reported_before_execution() {
    let cases = [
        (
            ["translations", "--target", "da", "--all-targets"].as_slice(),
            "choose --all-targets or --target",
        ),
        (
            ["translations", "--interactive", "--json"].as_slice(),
            "choose --interactive or --json",
        ),
        (
            ["translations", "--resolve", "confirm", "--apply"].as_slice(),
            "choose --resolve or --apply",
        ),
        (
            ["translations", "--resolve", "replace"].as_slice(),
            "--resolve replace requires --translation",
        ),
        (
            ["translations", "--translation", "tekst"].as_slice(),
            "require --resolve",
        ),
    ];

    for (args, expected) in cases {
        let output = run_vec(args);
        assert!(
            !output.status.success(),
            "args {args:?} unexpectedly succeeded"
        );
        assert!(
            stderr(&output).contains(expected),
            "args {args:?} stderr: {}",
            stderr(&output)
        );
    }
}

#[test]
fn multi_star_ignore_path_globs_filter_translation_coverage() {
    let dir = temp_dir("translations-multi-star-glob");
    write_workspace(&dir, "Helsinki", MULTI_STAR_IGNORE_OVERLAY);

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--status",
        "missing",
        "--full",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(!stdout(&output).contains("Helsinki"));
}

#[test]
fn stale_precedence_uses_direct_translation_while_reporting_stale_record() {
    let dir = temp_dir("translations-stale-precedence");
    write_workspace(&dir, "Helsinki City", SHADOWED_STALE_OVERLAY);

    let compose = run([
        "compose",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
    ]);
    assert!(compose.status.success(), "stderr: {}", stderr(&compose));
    assert!(stdout(&compose).contains("Helsingfors by"));
    assert!(!stdout(&compose).contains("Gammelt Helsingfors"));

    let report = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--status",
        "stale_translation",
        "--json",
    ]);
    assert!(report.status.success(), "stderr: {}", stderr(&report));
    let json: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(
        json["reports"][0]["entries"][0]["translated"],
        "Helsingfors by"
    );
}

#[test]
fn apply_rewrites_inline_empty_translations_map_without_duplicate_key() {
    let dir = temp_dir("translations-inline-empty-map");
    write_workspace(
        &dir,
        "Helsinki",
        r#"id: overlay.translation.da
kind: translation
translations: {} # empty scaffold
"#,
    );

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--source",
        "Helsinki",
        "--apply",
        "--full",
        "--no-interactive",
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));

    let overlay = fs::read_to_string(dir.join("da.yaml")).unwrap();
    assert_eq!(overlay.matches("translations:").count(), 1);
    assert!(!overlay.contains("translations: {}"));
    canonical_yaml::overlay_from_str(&overlay).expect("rewritten overlay parses");
}

fn write_workspace(dir: &Path, current_capital: &str, overlay: &str) {
    fs::write(
        dir.join("deck.yaml"),
        format!(
            r#"deck:
  id: deck.translation-cli
  name: Geography
  description: A fixture.
  adapter_ids: {{}}
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.country
      - field.capital
    fields:
      field.capital:
        name: Capital
      field.country:
        name: Country
    card_template_order:
      - template.capital
    card_templates:
      template.capital:
        name: Capital
        question_format: '{{{{Country}}}}'
        answer_format: '{{{{Capital}}}}'
        adapter_ids: {{}}
    styling: ''
    adapter_ids: {{}}
notes:
  note.finland:
    note_type_id: note-type.country
    fields:
      field.capital: {current_capital}
      field.country: Finland
    tags: []
    adapter_ids: {{}}
media: {{}}
tombstones: []
"#
        ),
    )
    .unwrap();
    fs::write(dir.join("da.yaml"), overlay).unwrap();
    fs::write(
        dir.join("brainbrew.yaml"),
        r#"base: deck.yaml
overlays:
  overlay.translation.da:
    file: da.yaml
    kind: translation
targets:
  da-release:
    overlays:
      - overlay.translation.da
    translation_coverage: strict
  da-standard:
    overlays:
      - overlay.translation.da
"#,
    )
    .unwrap();
}

fn run<const N: usize>(args: [&str; N]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .output()
        .expect("command runs")
}

fn run_vec(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .output()
        .expect("command runs")
}

fn command_text(output: &std::process::Output) -> String {
    format!("{}{}", stdout(output), stderr(output))
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn temp_dir(name: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    dir.push(format!("brain-brew-{name}-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

const STALE_ONLY_OVERLAY: &str = r#"id: overlay.translation.da
kind: translation
translations:
  ignore_paths:
    - 'deck.*'
    - 'note_types.*'
    - 'notes.*.fields.field.country'
stale_translations:
  - old_source: Helsinki
    new_source: Helsinki City
    target: Helsingfors
"#;

const SHADOWED_STALE_OVERLAY: &str = r#"id: overlay.translation.da
kind: translation
translations:
  ignore_paths:
    - 'deck.*'
    - 'note_types.*'
    - 'notes.*.fields.field.country'
  direct:
    Helsinki City: Helsingfors by
stale_translations:
  - old_source: Helsinki
    new_source: Helsinki City
    target: Gammelt Helsingfors
"#;

const MULTI_STAR_IGNORE_OVERLAY: &str = r#"id: overlay.translation.da
kind: translation
translations:
  ignore_paths:
    - 'deck.*'
    - 'note_types.*'
    - 'notes.*.fields.*capital'
  direct:
    Finland: Finland
"#;
