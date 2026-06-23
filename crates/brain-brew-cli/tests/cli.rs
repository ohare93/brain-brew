use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::Compression;
use flate2::write::GzEncoder;
use tar::Builder;

#[test]
fn top_level_help_includes_examples() {
    let output = run(["--help"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Usage:"));
    assert!(out.contains("Examples:"));
    assert!(out.contains("brainbrew targets --manifest brainbrew.yaml"));
    assert!(
        out.contains("brainbrew export crowdanki --manifest brainbrew.yaml --target de-extended")
    );
}

#[test]
fn command_help_includes_focused_examples() {
    let output = run(["compose", "--help"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Usage:"));
    assert!(out.contains(
        "brainbrew compose --manifest brainbrew.yaml --target da-standard --out build/da.yaml"
    ));
}

#[test]
fn validate_without_args_shows_usage_examples() {
    let output = run(["validate"]);

    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("Usage:"));
    assert!(err.contains("Examples:"));
    assert!(err.contains("brainbrew validate deck.yaml"));
}

#[test]
fn validate_reports_valid_deck_human_readably() {
    let dir = temp_dir("validate-valid");
    let deck_path = dir.join("deck.yaml");
    fs::write(&deck_path, SAMPLE_CANONICAL_YAML).unwrap();

    let output = run(["validate", deck_path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("✓"));
    assert!(out.contains("valid deck"));
    assert!(out.contains(deck_path.to_str().unwrap()));
    assert!(out.contains("notes: 1"));
}

#[test]
fn validate_reports_invalid_deck_path() {
    let dir = temp_dir("validate-invalid");
    let deck_path = dir.join("deck.yaml");
    fs::write(
        &deck_path,
        SAMPLE_CANONICAL_YAML.replace(
            "note_type_id: note-type.country",
            "note_type_id: note-type.missing",
        ),
    )
    .unwrap();

    let output = run(["validate", deck_path.to_str().unwrap()]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("notes.note.finland.note_type_id"));
}

#[test]
fn fmt_rewrites_canonical_yaml_in_place() {
    let dir = temp_dir("fmt");
    let deck_path = dir.join("deck.yaml");
    fs::write(&deck_path, MESSY_CANONICAL_YAML).unwrap();

    let output = run(["fmt", deck_path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(
        fs::read_to_string(deck_path).unwrap(),
        SAMPLE_CANONICAL_YAML
    );
}

#[test]
fn fmt_rewrites_overlay_yaml_in_place() {
    let dir = temp_dir("fmt-overlay");
    let overlay_path = dir.join("overlay.yaml");
    fs::write(&overlay_path, MESSY_OVERLAY_YAML).unwrap();

    let output = run(["fmt", overlay_path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(
        fs::read_to_string(overlay_path).unwrap(),
        CAPITAL_OVERLAY_YAML
    );
}

#[test]
fn fmt_rewrites_manifest_yaml_in_place() {
    let dir = temp_dir("fmt-manifest");
    let manifest_path = dir.join("brainbrew.yaml");
    fs::write(&manifest_path, MESSY_MANIFEST_YAML).unwrap();

    let output = run(["fmt", manifest_path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(fs::read_to_string(manifest_path).unwrap(), MANIFEST_YAML);
}

#[test]
fn fmt_rewrites_federation_lock_yaml_in_place() {
    let dir = temp_dir("fmt-lock");
    let lock_path = dir.join("brainbrew.lock");
    fs::write(&lock_path, MESSY_LOCK_YAML).unwrap();

    let output = run(["fmt", lock_path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(fs::read_to_string(lock_path).unwrap(), LOCK_YAML);
}

#[test]
fn compose_applies_overlay_files_in_order() {
    let dir = temp_dir("compose-overlay");
    let deck_path = dir.join("deck.yaml");
    let overlay_path = dir.join("overlay.yaml");
    let resolved_path = dir.join("resolved.yaml");
    fs::write(&deck_path, SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(&overlay_path, CAPITAL_OVERLAY_YAML).unwrap();

    let output = run([
        "compose",
        deck_path.to_str().unwrap(),
        "--overlay",
        overlay_path.to_str().unwrap(),
        "--out",
        resolved_path.to_str().unwrap(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("✓"));
    assert!(out.contains("composed deck"));
    assert!(out.contains(resolved_path.to_str().unwrap()));
    assert!(
        fs::read_to_string(resolved_path)
            .unwrap()
            .contains("field.capital: Helsingfors")
    );
}

#[test]
fn targets_lists_manifest_targets() {
    let dir = temp_dir("targets-manifest");
    write_manifest_workspace(&dir);

    let output = run([
        "targets",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "patched-via-dependency\n");
}

#[test]
fn targets_can_discover_multiple_package_manifests() {
    let first = temp_dir("targets-package-first");
    let second = temp_dir("targets-package-second");
    write_manifest_workspace(&first);
    write_manifest_workspace(&second);
    fs::write(first.join("brainbrew.yaml"), MANIFEST_WITH_PACKAGE_YAML).unwrap();
    fs::write(
        second.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML
            .replace("anki-geo.ultimate-geography", "anki-geo.rivers")
            .replace("patched-via-dependency", "rivers"),
    )
    .unwrap();

    let output = run([
        "targets",
        "--manifest",
        first.join("brainbrew.yaml").to_str().unwrap(),
        "--include",
        second.join("brainbrew.yaml").to_str().unwrap(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("anki-geo.ultimate-geography:patched-via-dependency"));
    assert!(out.contains("anki-geo.rivers:rivers"));
}

#[test]
fn targets_discovers_package_root_and_validates_dependencies() {
    let root = temp_dir("targets-package-root");
    let ug = root.join("ultimate-geography");
    let rivers = root.join("rivers");
    fs::create_dir_all(&ug).unwrap();
    fs::create_dir_all(&rivers).unwrap();
    write_manifest_workspace(&ug);
    write_manifest_workspace(&rivers);
    fs::write(
        ug.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML.replace("  depends_on:\n    - anki-geo.shared-geography\n", ""),
    )
    .unwrap();
    fs::write(
        rivers.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML
            .replace("anki-geo.ultimate-geography", "anki-geo.rivers")
            .replace(
                "depends_on:\n    - anki-geo.shared-geography",
                "depends_on:\n    - anki-geo.ultimate-geography@0.1.0",
            )
            .replace("patched-via-dependency", "rivers"),
    )
    .unwrap();

    let output = run(["targets", "--package-root", root.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("anki-geo.ultimate-geography:patched-via-dependency"));
    assert!(out.contains("anki-geo.rivers:rivers"));
}

#[test]
fn compose_can_resolve_extended_targets_from_brainbrew_lock() {
    let root = temp_dir("compose-federated-lock");
    let ug = root.join("ultimate-geography");
    let america = root.join("america");
    fs::create_dir_all(&ug).unwrap();
    fs::create_dir_all(&america).unwrap();
    write_manifest_workspace(&ug);
    fs::write(
        ug.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML.replace("  depends_on:\n    - anki-geo.shared-geography\n", ""),
    )
    .unwrap();
    fs::write(america.join("deck.yaml"), SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        america.join("america.yaml"),
        r#"id: overlay.extension.america
kind: extension
notes:
  note.finland:
    intent: merge
    tags:
      America::Imported:
        intent: add
"#,
    )
    .unwrap();
    fs::write(
        america.join("brainbrew.yaml"),
        r#"package:
  id: anki-geo.america
  version: 0.1.0
  depends_on:
    - anki-geo.ultimate-geography@0.1.0
base: deck.yaml
overlays:
  overlay.extension.america:
    file: america.yaml
    kind: extension
targets:
  en-america:
    extends: anki-geo.ultimate-geography:patched-via-dependency
    overlays:
      - overlay.extension.america
"#,
    )
    .unwrap();
    fs::write(
        america.join("brainbrew.lock"),
        format!(
            r#"version: 1
packages:
  anki-geo.ultimate-geography:
    manifest: brainbrew.yaml
    package:
      version: 0.1.0
    locked:
      type: path
      path: '{}'
"#,
            ug.canonicalize().unwrap().display()
        ),
    )
    .unwrap();
    let resolved = root.join("resolved.yaml");
    let cache = root.join("cache");

    let output = run_with_cache(
        [
            "compose",
            "--manifest",
            america.join("brainbrew.yaml").to_str().unwrap(),
            "--target",
            "en-america",
            "--out",
            resolved.to_str().unwrap(),
        ],
        &cache,
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let resolved_source = fs::read_to_string(resolved).unwrap();
    assert!(resolved_source.contains("field.capital: Helsingfors"));
    assert!(resolved_source.contains("America::Imported"));
}

#[test]
fn lock_update_and_verify_path_package_without_nix() {
    let root = temp_dir("lock-update-path");
    let ug = root.join("ultimate-geography");
    let america = root.join("america");
    fs::create_dir_all(&ug).unwrap();
    fs::create_dir_all(&america).unwrap();
    write_manifest_workspace(&ug);
    fs::write(
        ug.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML.replace("  depends_on:\n    - anki-geo.shared-geography\n", ""),
    )
    .unwrap();
    let lock_path = america.join("brainbrew.lock");

    let cache = root.join("cache");
    let update = run_with_cache(
        [
            "lock",
            "update",
            "--lock",
            lock_path.to_str().unwrap(),
            "--package",
            "anki-geo.ultimate-geography",
            "--path",
            ug.to_str().unwrap(),
        ],
        &cache,
    );

    assert!(update.status.success(), "stderr: {}", stderr(&update));
    let lock_source = fs::read_to_string(&lock_path).unwrap();
    assert!(lock_source.contains("original:\n      type: path"));
    assert!(lock_source.contains("locked:\n      type: path"));
    assert!(lock_source.contains(&format!("path: {}", ug.canonicalize().unwrap().display())));
    assert!(!lock_source.contains("/nix/store/"));
    assert!(lock_source.contains("nar_hash: 'sha256-"));

    let verify = run_with_cache(
        ["lock", "verify", "--lock", lock_path.to_str().unwrap()],
        &cache,
    );

    assert!(verify.status.success(), "stderr: {}", stderr(&verify));
    assert!(stdout(&verify).contains("verified 1 locked package"));

    fs::write(
        &lock_path,
        fs::read_to_string(&lock_path)
            .unwrap()
            .replace("sha256-", "sha256-bad"),
    )
    .unwrap();
    let mismatch = run_with_cache(
        ["lock", "verify", "--lock", lock_path.to_str().unwrap()],
        &cache,
    );

    assert!(!mismatch.status.success());
    assert!(stderr(&mismatch).contains("nar_hash mismatch"));
}

#[test]
fn lock_update_and_verify_tarball_package_without_nix() {
    let root = temp_dir("lock-update-tarball");
    let ug = root.join("ultimate-geography");
    let america = root.join("america");
    fs::create_dir_all(&ug).unwrap();
    fs::create_dir_all(&america).unwrap();
    write_manifest_workspace(&ug);
    fs::write(
        ug.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML.replace("  depends_on:\n    - anki-geo.shared-geography\n", ""),
    )
    .unwrap();
    let archive_path = root.join("ultimate-geography.tar.gz");
    write_tar_gz(&archive_path, "ultimate-geography", &ug);
    let lock_path = america.join("brainbrew.lock");
    let cache = root.join("cache");

    let update = run_with_cache(
        [
            "lock",
            "update",
            "--lock",
            lock_path.to_str().unwrap(),
            "--package",
            "anki-geo.ultimate-geography",
            "--tarball",
            &format!("file://{}", archive_path.display()),
        ],
        &cache,
    );

    assert!(update.status.success(), "stderr: {}", stderr(&update));
    let lock_source = fs::read_to_string(&lock_path).unwrap();
    assert!(lock_source.contains("original:\n      type: tarball"));
    assert!(lock_source.contains("locked:\n      type: tarball"));
    assert!(lock_source.contains("nar_hash: 'sha256-"));

    let verify = run_with_cache(
        ["lock", "verify", "--lock", lock_path.to_str().unwrap()],
        &cache,
    );

    assert!(verify.status.success(), "stderr: {}", stderr(&verify));
    assert!(stdout(&verify).contains("verified 1 locked package"));
}

#[test]
fn compose_can_extend_targets_and_mix_overlays_from_included_package_manifests() {
    let root = temp_dir("compose-federated-package");
    let ug = root.join("ultimate-geography");
    let america = root.join("america");
    fs::create_dir_all(&ug).unwrap();
    fs::create_dir_all(&america).unwrap();
    write_manifest_workspace(&ug);
    fs::write(
        ug.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML.replace("  depends_on:\n    - anki-geo.shared-geography\n", ""),
    )
    .unwrap();
    fs::write(america.join("deck.yaml"), SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        america.join("america.yaml"),
        r#"id: overlay.extension.america
kind: extension
notes:
  note.finland:
    intent: merge
    tags:
      America::Imported:
        intent: add
"#,
    )
    .unwrap();
    fs::write(
        america.join("brainbrew.yaml"),
        r#"package:
  id: anki-geo.america
  version: 0.1.0
  depends_on:
    - anki-geo.ultimate-geography@0.1.0
base: deck.yaml
overlays:
  overlay.extension.america:
    file: america.yaml
    kind: extension
targets:
  en-america:
    extends: anki-geo.ultimate-geography:patched-via-dependency
    overlays:
      - overlay.extension.america
"#,
    )
    .unwrap();
    let america_manifest = america.join("brainbrew.yaml");
    let ug_manifest = ug.join("brainbrew.yaml");
    let resolved = root.join("resolved.yaml");

    let output = run([
        "compose",
        "--manifest",
        america_manifest.to_str().unwrap(),
        "--include",
        ug_manifest.to_str().unwrap(),
        "--target",
        "en-america",
        "--out",
        resolved.to_str().unwrap(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let resolved_source = fs::read_to_string(resolved).unwrap();
    assert!(resolved_source.contains("field.capital: Helsingfors"));
    assert!(resolved_source.contains("America::Imported"));

    let mixer = root.join("mixer");
    fs::create_dir_all(&mixer).unwrap();
    fs::write(mixer.join("deck.yaml"), SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        mixer.join("brainbrew.yaml"),
        r#"package:
  id: example.mix
  version: 0.1.0
  depends_on:
    - anki-geo.ultimate-geography@0.1.0
    - anki-geo.america@0.1.0
base: deck.yaml
overlays: {}
targets:
  en-mixed:
    extends: anki-geo.ultimate-geography:patched-via-dependency
    overlays:
      - anki-geo.america:overlay.extension.america
"#,
    )
    .unwrap();
    let mixer_manifest = mixer.join("brainbrew.yaml");
    let mixed_resolved = root.join("mixed-resolved.yaml");

    let output = run([
        "compose",
        "--manifest",
        mixer_manifest.to_str().unwrap(),
        "--include",
        ug_manifest.to_str().unwrap(),
        "--include",
        america_manifest.to_str().unwrap(),
        "--target",
        "en-mixed",
        "--out",
        mixed_resolved.to_str().unwrap(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let mixed_source = fs::read_to_string(mixed_resolved).unwrap();
    assert!(mixed_source.contains("field.capital: Helsingfors"));
    assert!(mixed_source.contains("America::Imported"));
}

#[test]
fn targets_reports_missing_package_dependencies() {
    let root = temp_dir("targets-missing-package-dep");
    let rivers = root.join("rivers");
    fs::create_dir_all(&rivers).unwrap();
    write_manifest_workspace(&rivers);
    fs::write(
        rivers.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML
            .replace("anki-geo.ultimate-geography", "anki-geo.rivers")
            .replace(
                "depends_on:\n    - anki-geo.shared-geography",
                "depends_on:\n    - anki-geo.ultimate-geography@0.1.0",
            )
            .replace("patched-via-dependency", "rivers"),
    )
    .unwrap();

    let output = run(["targets", "--package-root", root.to_str().unwrap()]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("package dependency anki-geo.ultimate-geography"));
}

#[test]
fn targets_reports_package_dependency_version_mismatches() {
    let root = temp_dir("targets-package-version-mismatch");
    let ug = root.join("ultimate-geography");
    let rivers = root.join("rivers");
    fs::create_dir_all(&ug).unwrap();
    fs::create_dir_all(&rivers).unwrap();
    write_manifest_workspace(&ug);
    write_manifest_workspace(&rivers);
    fs::write(
        ug.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML.replace("  depends_on:\n    - anki-geo.shared-geography\n", ""),
    )
    .unwrap();
    fs::write(
        rivers.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML
            .replace("anki-geo.ultimate-geography", "anki-geo.rivers")
            .replace(
                "depends_on:\n    - anki-geo.shared-geography",
                "depends_on:\n    - anki-geo.ultimate-geography@9.9.9",
            )
            .replace("patched-via-dependency", "rivers"),
    )
    .unwrap();

    let output = run(["targets", "--package-root", root.to_str().unwrap()]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("resolved to version 0.1.0"));
}

#[test]
fn targets_json_includes_package_metadata() {
    let dir = temp_dir("targets-package-json");
    write_manifest_workspace(&dir);
    fs::write(dir.join("brainbrew.yaml"), MANIFEST_WITH_PACKAGE_YAML).unwrap();

    let output = run([
        "targets",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--json",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(json["package"]["id"], "anki-geo.ultimate-geography");
    assert_eq!(json["package"]["version"], "0.1.0");
}

#[test]
fn targets_can_report_json_with_expanded_overlays() {
    let dir = temp_dir("targets-json");
    write_manifest_workspace(&dir);

    let output = run([
        "targets",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--json",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(json["targets"][0]["name"], "patched-via-dependency");
    assert_eq!(json["targets"][0]["overlays"][0]["id"], "patch.capital");
    assert_eq!(
        json["targets"][0]["overlays"][1]["id"],
        "noop.after-capital"
    );
}

#[test]
fn validate_uses_manifest_target() {
    let dir = temp_dir("validate-manifest");
    write_manifest_workspace(&dir);

    let output = run([
        "validate",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("✓"));
    assert!(out.contains("valid target"));
    assert!(out.contains("patched-via-dependency"));
}

#[test]
fn manifest_target_errors_list_available_targets() {
    let dir = temp_dir("missing-target");
    write_manifest_workspace(&dir);

    let output = run([
        "compose",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "missing",
    ]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("available targets: patched-via-dependency"));
}

#[test]
fn compose_uses_manifest_target_dependency_expansion() {
    let dir = temp_dir("compose-manifest");
    write_manifest_workspace(&dir);
    let resolved_path = dir.join("resolved.yaml");

    let output = run([
        "compose",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
        "--out",
        resolved_path.to_str().unwrap(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("✓"));
    assert!(out.contains("composed target"));
    assert!(out.contains("patched-via-dependency"));
    assert!(out.contains(resolved_path.to_str().unwrap()));
    assert!(
        fs::read_to_string(resolved_path)
            .unwrap()
            .contains("field.capital: Helsingfors")
    );
}

#[test]
fn export_crowdanki_uses_manifest_target_configured_out() {
    let dir = temp_dir("export-manifest-configured-out");
    write_manifest_workspace(&dir);
    fs::write(dir.join("brainbrew.yaml"), MANIFEST_WITH_EXPORTS_YAML).unwrap();

    let output = run([
        "export",
        "crowdanki",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(dir.join("configured-crowdanki/deck.json").exists());
}

#[test]
fn export_crowdanki_defaults_manifest_target_out_to_build_crowdanki_target() {
    let dir = temp_dir("export-manifest-default-out");
    write_manifest_workspace(&dir);

    let output = run([
        "export",
        "crowdanki",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(
        dir.join("build/crowdanki/patched-via-dependency/deck.json")
            .exists()
    );
}

#[test]
fn export_crowdanki_uses_manifest_target() {
    let dir = temp_dir("export-manifest");
    write_manifest_workspace(&dir);
    let export_dir = dir.join("crowdanki");

    let output = run([
        "export",
        "crowdanki",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
        "--out",
        export_dir.to_str().unwrap(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(
        fs::read_to_string(export_dir.join("deck.json"))
            .unwrap()
            .contains("Helsingfors")
    );
}

#[test]
fn verify_compares_configured_crowdanki_golden() {
    let dir = temp_dir("verify-golden");
    write_manifest_workspace(&dir);
    fs::write(dir.join("brainbrew.yaml"), MANIFEST_WITH_EXPORTS_YAML).unwrap();
    let golden_dir = dir.join("goldens/patched");

    let export_output = run([
        "export",
        "crowdanki",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
        "--out",
        golden_dir.to_str().unwrap(),
    ]);
    assert!(
        export_output.status.success(),
        "stderr: {}",
        stderr(&export_output)
    );

    let verify_output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
    ]);
    assert!(
        verify_output.status.success(),
        "stderr: {}",
        stderr(&verify_output)
    );

    let golden_path = golden_dir.join("deck.json");
    fs::write(
        &golden_path,
        fs::read_to_string(&golden_path)
            .unwrap()
            .replace("Helsingfors", "Helsinki"),
    )
    .unwrap();
    let mismatch_output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
    ]);
    assert!(!mismatch_output.status.success());
    assert!(stderr(&mismatch_output).contains("CrowdAnki golden mismatch"));
}

#[test]
fn verify_allows_configured_crowdanki_golden_paths() {
    let dir = temp_dir("verify-golden-allowlist");
    write_manifest_workspace(&dir);
    fs::write(
        dir.join("brainbrew.yaml"),
        MANIFEST_WITH_EXPORTS_YAML.replace(
            "        golden: goldens/patched/deck.json\n",
            "        golden: goldens/patched/deck.json\n        golden_allowlist:\n          - '$.name'\n",
        ),
    )
    .unwrap();
    let golden_dir = dir.join("goldens/patched");

    let export_output = run([
        "export",
        "crowdanki",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
        "--out",
        golden_dir.to_str().unwrap(),
    ]);
    assert!(
        export_output.status.success(),
        "stderr: {}",
        stderr(&export_output)
    );

    let golden_path = golden_dir.join("deck.json");
    let mut golden_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&golden_path).unwrap()).unwrap();
    golden_json["name"] = serde_json::json!("Allowed Legacy Name");
    fs::write(
        &golden_path,
        serde_json::to_string_pretty(&golden_json).unwrap(),
    )
    .unwrap();

    let verify_output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
    ]);
    assert!(
        verify_output.status.success(),
        "stderr: {}",
        stderr(&verify_output)
    );
}

#[test]
fn verify_checks_all_manifest_targets() {
    let dir = temp_dir("verify-manifest");
    write_manifest_workspace(&dir);

    let output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("✓"));
    assert!(out.contains("verified 1 target"));
}

#[test]
fn translate_aliases_run_translation_reports() {
    let dir = temp_dir("translate-aliases");
    write_translation_workspace(&dir);

    for command in ["translate", "translation"] {
        let output = run([
            command,
            "--manifest",
            dir.join("brainbrew.yaml").to_str().unwrap(),
            "--target",
            "da-standard",
            "--json",
        ]);

        assert!(
            output.status.success(),
            "{command} stderr: {}",
            stderr(&output)
        );
        let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
        assert_eq!(json["reports"][0]["target"], "da-standard");
    }
}

#[test]
fn unknown_translate_like_command_suggests_translations() {
    let output = run(["translatons"]);

    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("unknown command"));
    assert!(err.contains("Did you mean:"));
    assert!(err.contains("brainbrew translations"));
}

#[test]
fn translations_missing_manifest_lists_nearby_manifests() {
    let dir = temp_dir("translations-missing-manifest");
    let workspace = dir.join("decks/sample");
    fs::create_dir_all(&workspace).unwrap();
    write_translation_workspace(&workspace);

    let output = run_in_dir(["translations", "--no-interactive"], &dir);

    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("No Brain Brew manifest found at brainbrew.yaml"));
    assert!(err.contains("Found possible manifests:"));
    assert!(err.contains("decks/sample/brainbrew.yaml"));
    assert!(err.contains("brainbrew translations --manifest decks/sample/brainbrew.yaml"));
}

#[test]
fn translations_reports_when_selected_target_has_no_dictionary_overlays() {
    let manifest = workspace_root().join("fixtures/ug-style/brainbrew.yaml");
    let output = run([
        "translations",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "full-demo",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("No translation dictionary coverage reports matched"));
    assert!(out.contains("translation.es"));
    assert!(out.contains("translation-sv.yaml"));
    assert!(out.contains("do not use a `translations:` dictionary"));
}

#[test]
fn translations_context_view_shows_missing_note_field_card_context() {
    let dir = temp_dir("translations-context-missing");
    write_translation_workspace(&dir);

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--context",
        "--source",
        "Sweden",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Translation context for target da-standard language da"));
    assert!(out.contains("missing_translation notes.note.sweden.fields.field.country"));
    assert!(out.contains("note: note.sweden"));
    assert!(out.contains("field: field.country (Country)"));
    assert!(out.contains("note fields (source/en | target/da):"));
    assert!(out.contains("* field.country (Country)"));
    assert!(out.contains("cards: template.country-capital [question]"));
    assert!(out.contains("source/en"));
    assert!(out.contains("target/da"));
    assert!(out.contains("Sweden"));
}

#[test]
fn translations_context_view_shows_ug_repeated_country_info_occurrences() {
    let manifest = workspace_root().join("fixtures/ultimate-geography/brainbrew.yaml");
    let output = run([
        "translations",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "es-standard",
        "--overlay",
        "overlay.translation.es",
        "--context",
        "--source",
        "Island of Indonesia.",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("duplicate source group: 3 occurrence(s)"));
    assert!(out.contains("note: note.bali"));
    assert!(out.contains("note: note.java"));
    assert!(out.contains("note: note.sumatra"));
    assert!(out.contains("field: field.country-info (Country info)"));
    assert!(out.contains("template.capital-country [answer]"));
    assert!(out.contains("template.country-capital [question+answer]"));
    assert!(out.contains("Island of Indonesia."));
    assert!(out.contains("Isla de Indonesia."));
}

#[test]
fn translations_context_view_wraps_long_ug_flag_similarity_context() {
    let manifest = workspace_root().join("fixtures/ultimate-geography/brainbrew.yaml");
    let output = run([
        "translations",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "es-standard",
        "--overlay",
        "overlay.translation.es",
        "--context",
        "--source",
        "blue background, red and white cross",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("note: note.faroe-islands"));
    assert!(out.contains("field: field.flag-similarity (Flag similarity)"));
    assert!(out.contains("template.flag-country [answer]"));
    assert!(out.contains("Iceland (blue background,"));
    assert!(out.contains("Islandia (fondo azul,"));
    assert!(out.contains("Noruega"));
}

#[test]
fn translations_context_view_json_exposes_reusable_context_model() {
    let manifest = workspace_root().join("fixtures/ultimate-geography/brainbrew.yaml");
    let output = run([
        "translations",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "es-standard",
        "--overlay",
        "overlay.translation.es",
        "--context",
        "--source",
        "Autonomous community of Spain.",
        "--json",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    let unit = &json["contexts"][0]["units"][0];
    assert_eq!(unit["status"], "direct_translation");
    assert_eq!(unit["note_id"], "note.canary-islands");
    assert_eq!(unit["field_id"], "field.country-info");
    assert_eq!(unit["field_name"], "Country info");
    assert_eq!(unit["note_fields"][0]["field_id"], "field.country");
    assert_eq!(unit["note_fields"][0]["source"], "Canary Islands");
    assert_eq!(unit["note_fields"][0]["translated"], "Canarias");
    assert_eq!(unit["source"], "Autonomous community of Spain.");
    assert_eq!(unit["translated"], "Comunidad autónoma de España.");
    assert!(unit["card_templates"].as_array().unwrap().len() >= 4);
}

#[test]
fn translations_context_view_json_shows_structured_message_components() {
    let dir = temp_dir("translations-context-structured-message");
    write_structured_message_translation_workspace(&dir);

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "nb-standard",
        "--context",
        "--source",
        "blue background",
        "--json",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    let unit = &json["contexts"][0]["units"][0];
    assert_eq!(
        unit["path"],
        "notes.note.finland.fields.field.flag-similarity.message.2"
    );
    assert_eq!(unit["status"], "contextual_override");
    assert_eq!(
        unit["message"]["source"],
        "Iceland (blue background with a white cross), Norway (red background with a blue cross)"
    );
    assert_eq!(
        unit["message"]["translated"],
        "Island (blå bakgrunn med hvitt kors), Norge (rød bakgrunn med blått kors)"
    );
    let components = unit["message"]["components"].as_array().unwrap();
    assert_eq!(components[0]["kind"], "field_ref");
    assert_eq!(
        components[0]["reference"],
        "notes.note.iceland.fields.field.country"
    );
    assert_eq!(components[0]["source"], "Iceland");
    assert_eq!(components[0]["translated"], "Island");
    assert_eq!(components[2]["kind"], "text");
    assert_eq!(
        components[2]["source"],
        "blue background with a white cross"
    );
    assert_eq!(components[2]["translated"], "blå bakgrunn med hvitt kors");
}

#[test]
fn translations_context_apply_uses_existing_translation_apply_path() {
    let dir = temp_dir("translations-context-apply");
    write_translation_workspace(&dir);
    let overlay_path = dir.join("da.yaml");

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--context",
        "--source",
        "Sweden",
        "--apply",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let updated = fs::read_to_string(overlay_path).unwrap();
    assert!(updated.contains("    Sweden: Sweden\n"));
}

#[test]
fn translations_context_view_can_filter_duplicate_missing_and_language() {
    let dir = temp_dir("translations-context-filters");
    write_translation_workspace(&dir);
    let deck_path = dir.join("deck.yaml");
    let deck = fs::read_to_string(&deck_path)
        .unwrap()
        .replace("field.country: Finland", "field.country: Sweden");
    fs::write(&deck_path, deck).unwrap();

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
        "--language",
        "da",
        "--context",
        "--duplicates",
        "--status",
        "missing",
        "--field",
        "field.country",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("missing_translation notes.note.sweden.fields.field.country"));
    assert!(out.contains("duplicate source group: 2 occurrence(s)"));
    assert!(!out.contains("notes.note.finland.fields.field.capital"));
}

#[test]
fn translations_summary_exports_compact_counts_by_language() {
    let dir = temp_dir("translations-summary");
    write_translation_workspace(&dir);

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
        "--summary",
        "--json",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert!(json.get("reports").is_none());
    let summaries = json["summaries"].as_array().unwrap();
    assert_eq!(summaries.len(), 1);
    let row = &summaries[0];
    assert_eq!(row["language"], "da");
    assert_eq!(
        row["targets"],
        serde_json::json!(["da-release", "da-standard"])
    );
    assert_eq!(row["overlay"], "overlay.translation.da");
    assert_eq!(row["file"], "da.yaml");
    assert_eq!(row["direct_translation"], 1);
    assert_eq!(row["contextual_override"], 1);
    assert_eq!(row["no_change"], 0);
    assert_eq!(row["target_language_addition"], 1);
    assert_eq!(row["variable_translation"], 0);
    assert_eq!(row["adapter_id_translation"], 0);
    assert_eq!(row["untranslated_fallback"], 2);
    assert_eq!(row["missing_text_translation"], 2);
    assert_eq!(row["hidden_untranslated_fallback"], 0);
    assert_eq!(row["stale_invalid"], 1);
}

#[test]
fn translations_summary_human_output_uses_aligned_space_columns() {
    let dir = temp_dir("translations-summary-human");
    write_translation_workspace(&dir);

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
        "--summary",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(
        !out.contains('\t'),
        "summary should not rely on terminal tab stops:\n{out}"
    );
    assert!(
        out.contains("lang  tgt  direct"),
        "header should use compact padded columns:\n{out}"
    );
    assert!(
        !out.contains("overlay.translation.da"),
        "default summary should keep wide overlay/file columns out of the table:\n{out}"
    );
    for line in out.lines().skip(1) {
        assert!(
            line.chars().count() <= 90,
            "default summary line should stay compact, got {} chars:\n{line}\n\n{out}",
            line.chars().count()
        );
    }
    let full = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
        "--summary",
        "--full",
    ]);
    assert!(full.status.success(), "stderr: {}", stderr(&full));
    assert!(stdout(&full).contains("overlay.translation.da"));
    assert!(stdout(&full).contains("da.yaml"));
}

#[test]
fn translations_default_report_focuses_on_translatable_note_text() {
    let manifest = workspace_root().join("fixtures/ultimate-geography/brainbrew.yaml");
    let output = run([
        "translations",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "da-standard",
        "--overlay",
        "overlay.translation.da",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("missing text translations: 0"));
    assert!(out.contains("intentionally unchanged text: 329"));
    assert!(out.contains("hidden structural/media/tag fallbacks:"));
    assert!(out.contains("hint: use --full"));
    assert!(!out.contains("deck.description source="));
    assert!(!out.contains("notes.note.abkhazia.fields.field.flag"));
    assert!(!out.contains("notes.note.abkhazia.fields.field.capital"));
}

#[test]
fn ultimate_geography_language_overlays_have_no_actionable_missing_text() {
    let manifest = workspace_root().join("fixtures/ultimate-geography/brainbrew.yaml");
    let output = run([
        "translations",
        "--manifest",
        manifest.to_str().unwrap(),
        "--all-targets",
        "--json",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    let mut missing = Vec::new();
    for report in json["reports"].as_array().unwrap() {
        let file = report["file"].as_str().unwrap();
        if !file.starts_with("overlays/languages/") {
            continue;
        }
        let target = report["target"].as_str().unwrap();
        let overlay = report["overlay"].as_str().unwrap();
        for entry in report["entries"].as_array().unwrap() {
            if entry["category"] != "untranslated_fallback" {
                continue;
            }
            let path = entry["path"].as_str().unwrap();
            let source = entry["source"].as_str().unwrap();
            let actionable_note_field = path.starts_with("notes.")
                && path.contains(".fields.")
                && !path.ends_with(".fields.field.flag")
                && !path.ends_with(".fields.field.map")
                && !source.trim_start().starts_with("<img");
            if actionable_note_field {
                missing.push(format!("{target} {overlay} {path} source={source:?}"));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "language overlays should use translations or no_change for reviewed note text:\n{}",
        missing.join("\n")
    );
}

#[test]
fn translations_full_report_includes_structural_fallbacks() {
    let manifest = workspace_root().join("fixtures/ultimate-geography/brainbrew.yaml");
    let output = run([
        "translations",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "da-standard",
        "--overlay",
        "overlay.translation.da",
        "--full",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("deck.description source="));
    assert!(!out.contains("hidden structural/media/tag fallbacks:"));
}

#[test]
fn translations_interactive_language_options_come_from_translation_overlays_only() {
    let manifest = workspace_root().join("fixtures/ug-style/brainbrew.yaml");
    let output = run_with_stdin(
        [
            "translate",
            "--manifest",
            manifest.to_str().unwrap(),
            "--interactive",
        ],
        "\x1b[B\nq",
    );

    assert!(!output.status.success());
    let out = stdout(&output);
    assert!(out.contains("Language filter"));
    assert!(out.contains("es"));
    assert!(out.contains("sv"));
    assert!(!out.contains("capital"));
    assert!(!out.contains("hint"));
}

#[test]
fn translations_human_output_can_be_colored_but_json_stays_plain() {
    let dir = temp_dir("translations-color");
    write_translation_workspace(&dir);
    let manifest = dir.join("brainbrew.yaml");

    let colored = run_with_env(
        [
            "translations",
            "--manifest",
            manifest.to_str().unwrap(),
            "--target",
            "da-standard",
        ],
        &[("BRAINBREW_COLOR", "always")],
    );
    assert!(colored.status.success(), "stderr: {}", stderr(&colored));
    assert!(stdout(&colored).contains("\u{1b}["));

    let json = run_with_env(
        [
            "translations",
            "--manifest",
            manifest.to_str().unwrap(),
            "--target",
            "da-standard",
            "--json",
        ],
        &[("BRAINBREW_COLOR", "always")],
    );
    assert!(json.status.success(), "stderr: {}", stderr(&json));
    assert!(!stdout(&json).contains("\u{1b}["));
}

#[test]
fn translations_interactive_derives_selector_options_and_prints_equivalent_command() {
    let dir = temp_dir("translations-interactive-selectors");
    write_translation_workspace(&dir);

    let output = run_with_stdin(
        [
            "translate",
            "--manifest",
            dir.join("brainbrew.yaml").to_str().unwrap(),
            "--interactive",
        ],
        "\x1b[B\n\n\n\n\n",
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Brain Brew translation coverage"));
    assert!(out.contains("Target"));
    assert!(out.contains("da-release"));
    assert!(out.contains("da-standard"));
    assert!(!out.contains("Language filter"));
    assert!(!out.contains("Translation overlay"));
    assert!(out.contains("Scope"));
    assert!(out.contains("Equivalent command:"));
    assert!(out.contains("brainbrew translations"));
    assert!(out.contains("--target da-standard"));
    assert!(!out.contains("--language"));
    assert!(!out.contains("--overlay"));
    assert!(!out.contains("Select Target"));
    assert!(!out.contains("1. da-release"));
}

#[test]
fn translations_interactive_apply_can_mark_all_selected_rows_no_change() {
    let dir = temp_dir("translations-interactive-no-change-all");
    write_translation_workspace(&dir);
    let overlay_path = dir.join("da.yaml");

    let output = run_with_stdin(
        [
            "translations",
            "--manifest",
            dir.join("brainbrew.yaml").to_str().unwrap(),
            "--target",
            "da-standard",
            "--path-prefix",
            "notes.note.sweden.fields.field",
            "--apply",
            "--interactive",
        ],
        "\n\n\n",
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Action for selected missing translations"));
    assert!(out.contains("mark no-change for all 2 selected missing translations"));
    let updated = fs::read_to_string(overlay_path).unwrap();
    assert!(updated.contains("  no_change:\n"));
    assert!(updated.contains("    - Stockholm\n"));
    assert!(updated.contains("    - Sweden\n"));
}

#[test]
fn translations_interactive_apply_can_use_one_translation_stub_action_for_all_selected_rows() {
    let dir = temp_dir("translations-interactive-direct-all");
    write_translation_workspace(&dir);
    let overlay_path = dir.join("da.yaml");

    let output = run_with_stdin(
        [
            "translations",
            "--manifest",
            dir.join("brainbrew.yaml").to_str().unwrap(),
            "--target",
            "da-standard",
            "--path-prefix",
            "notes.note.sweden.fields.field",
            "--apply",
            "--interactive",
        ],
        "\n\x1b[B\n\n",
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains(
        "add direct source→source translation stubs for all 2 selected missing translations"
    ));
    let updated = fs::read_to_string(overlay_path).unwrap();
    assert!(updated.contains("    Stockholm: Stockholm\n"));
    assert!(updated.contains("    Sweden: Sweden\n"));
}

#[test]
fn translations_interactive_apply_can_insert_contextual_stub() {
    let dir = temp_dir("translations-interactive-contextual");
    write_translation_workspace(&dir);
    let overlay_path = dir.join("da.yaml");

    let output = run_with_stdin(
        [
            "translations",
            "--manifest",
            dir.join("brainbrew.yaml").to_str().unwrap(),
            "--target",
            "da-standard",
            "--path-prefix",
            "notes.note.sweden.fields.field.country",
            "--apply",
            "--interactive",
        ],
        "\n\x1b[B\x1b[B\n\n",
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let updated = fs::read_to_string(overlay_path).unwrap();
    assert!(updated.contains("    notes.note.sweden:\n      Sweden: Sweden\n"));
}

#[test]
fn translations_interactive_apply_can_insert_ignore_path() {
    let dir = temp_dir("translations-interactive-ignore");
    write_translation_workspace(&dir);
    let overlay_path = dir.join("da.yaml");

    let output = run_with_stdin(
        [
            "translations",
            "--manifest",
            dir.join("brainbrew.yaml").to_str().unwrap(),
            "--target",
            "da-standard",
            "--path-prefix",
            "notes.note.sweden.fields.field.country",
            "--apply",
            "--interactive",
        ],
        "\n\x1b[B\x1b[B\x1b[B\n\n",
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let updated = fs::read_to_string(overlay_path).unwrap();
    assert!(updated.contains("  ignore_paths:\n"));
    assert!(updated.contains("    - notes.note.sweden.fields.field.country\n"));
}

#[test]
fn translations_report_counts_no_change_as_intentionally_unchanged_text() {
    let dir = temp_dir("translations-no-change-report");
    write_translation_workspace(&dir);
    fs::write(
        dir.join("da.yaml"),
        r#"id: overlay.translation.da
kind: translation
translations:
  ignore_paths:
    - deck.*
    - note_types.*
    - notes.*.tags.*
    - notes.*.fields.field.flag
  no_change:
    - Stockholm
    - Sweden
"#,
    )
    .unwrap();

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--path-prefix",
        "notes.note.sweden.fields.field",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("intentionally unchanged text: 2"));
    assert!(out.contains("missing text translations: 0"));
    assert!(!out.contains("missing_translation notes.note.sweden.fields.field.country"));
}

#[test]
fn translations_reports_missing_stale_contextual_and_additions_without_modifying() {
    let dir = temp_dir("translations-report");
    write_translation_workspace(&dir);
    let overlay_path = dir.join("da.yaml");
    let before = fs::read_to_string(&overlay_path).unwrap();

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Translation coverage for target da-standard"));
    assert!(out.contains("contextual overrides: 1"));
    assert!(out.contains("target-language additions: 1"));
    assert!(out.contains("stale_direct_key translations.direct.Removed source"));
    assert!(out.contains("missing_translation notes.note.sweden.fields.field.country"));
    assert_eq!(fs::read_to_string(overlay_path).unwrap(), before);
}

#[test]
fn translations_apply_inserts_sorted_direct_stubs_and_preserves_comments() {
    let dir = temp_dir("translations-apply");
    write_translation_workspace(&dir);
    let overlay_path = dir.join("da.yaml");

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--path-prefix",
        "notes.note.sweden.fields.field.country",
        "--apply",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let updated = fs::read_to_string(overlay_path).unwrap();
    assert!(updated.contains("# translator note kept"));
    assert!(updated.contains("    Sweden: Sweden\n"));
}

#[test]
fn verify_translation_coverage_policy_can_be_lenient_or_strict() {
    let dir = temp_dir("translations-verify-policy");
    write_translation_workspace(&dir);
    let overlay_path = dir.join("da.yaml");
    fs::write(
        &overlay_path,
        r#"id: overlay.translation.da
kind: translation
translations:
  ignore_paths:
    - 'deck.*'
    - 'note_types.*'
    - 'notes.*.fields.field.flag'
    - 'notes.*.tags.*'
  direct:
    Finland: Finland
  contextual:
    notes.note:
      finland:
        Helsinki: Helsingfors
  target_additions:
    notes.note.finland.fields.field.flag: '<img src="fi-da.png">'
"#,
    )
    .unwrap();

    let lenient_output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--translation-coverage",
        "lenient",
    ]);
    assert!(
        lenient_output.status.success(),
        "stderr: {}",
        stderr(&lenient_output)
    );

    let strict_output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-release",
    ]);
    assert!(!strict_output.status.success());
    assert!(stderr(&strict_output).contains("translation coverage strict policy failed"));
    assert!(stderr(&strict_output).contains("notes.note.sweden.fields.field.country"));

    fs::write(
        &overlay_path,
        r#"id: overlay.translation.da
kind: translation
translations:
  ignore_paths:
    - 'deck.*'
    - 'note_types.*'
    - 'notes.*.fields.field.flag'
    - 'notes.*.tags.*'
  direct:
    Finland: Finland
  contextual:
    notes.note:
      finland:
        Helsinki: Helsingfors
  no_change:
    - Stockholm
    - Sweden
  target_additions:
    notes.note.finland.fields.field.flag: '<img src="fi-da.png">'
"#,
    )
    .unwrap();

    let strict_output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-release",
    ]);
    assert!(
        strict_output.status.success(),
        "stderr: {}",
        stderr(&strict_output)
    );
}

#[test]
fn export_crowdanki_copies_media_from_media_root_and_checks_hashes() {
    let dir = temp_dir("export-media");
    let deck_path = dir.join("deck.yaml");
    let media_root = dir.join("media");
    let export_dir = dir.join("crowdanki");
    fs::write(&deck_path, MEDIA_CANONICAL_YAML).unwrap();
    fs::create_dir_all(media_root.join("flags")).unwrap();
    fs::write(media_root.join("flags/fi.png"), b"flag-bytes").unwrap();

    let output = run([
        "export",
        "crowdanki",
        deck_path.to_str().unwrap(),
        "--media-root",
        media_root.to_str().unwrap(),
        "--out",
        export_dir.to_str().unwrap(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(
        fs::read(export_dir.join("media/flags/fi.png")).unwrap(),
        b"flag-bytes"
    );
}

#[test]
fn verify_checks_media_root_hashes() {
    let dir = temp_dir("verify-media");
    fs::write(dir.join("deck.yaml"), MEDIA_CANONICAL_YAML).unwrap();
    fs::write(dir.join("brainbrew.yaml"), SIMPLE_MEDIA_MANIFEST_YAML).unwrap();
    fs::create_dir_all(dir.join("media/flags")).unwrap();
    fs::write(dir.join("media/flags/fi.png"), b"wrong-bytes").unwrap();

    let output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
        "--media-root",
        dir.join("media").to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("sha256"));
}

#[test]
fn explain_reports_expanded_stack_and_diff() {
    let dir = temp_dir("explain");
    write_manifest_workspace(&dir);

    let output = run([
        "explain",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("target: patched-via-dependency"));
    assert!(out.contains("1. patch.capital (capital.yaml)"));
    assert!(out.contains("modified notes.note.finland.fields.field.capital"));
}

#[test]
fn explain_reports_json_for_ui_consumers() {
    let dir = temp_dir("explain-json");
    write_manifest_workspace(&dir);
    fs::write(dir.join("brainbrew.yaml"), MANIFEST_WITH_PACKAGE_YAML).unwrap();

    let output = run([
        "explain",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
        "--json",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(json["package"]["id"], "anki-geo.ultimate-geography");
    assert_eq!(json["target"], "patched-via-dependency");
    assert_eq!(json["overlay_stack"][0]["id"], "patch.capital");
    assert_eq!(
        json["changes"][0]["path"],
        "notes.note.finland.fields.field.capital"
    );
}

#[test]
fn explain_reports_json_conflicts_for_ui_consumers() {
    let dir = temp_dir("explain-conflict-json");
    write_manifest_workspace(&dir);
    fs::write(dir.join("second.yaml"), SECOND_CAPITAL_OVERLAY_YAML).unwrap();
    fs::write(dir.join("brainbrew.yaml"), CONFLICT_MANIFEST_YAML).unwrap();

    let output = run([
        "explain",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "conflict",
        "--json",
    ]);

    assert!(!output.status.success());
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(json["target"], "conflict");
    assert_eq!(json["overlay_stack"][1]["id"], "patch.capital.second");
    assert_eq!(json["errors"][0]["kind"], "Conflict");
    assert_eq!(
        json["errors"][0]["path"],
        "notes.note.finland.fields.field.capital"
    );
}

#[test]
fn explain_reports_overlay_conflicts_with_stack_context() {
    let dir = temp_dir("explain-conflict");
    write_manifest_workspace(&dir);
    fs::write(dir.join("second.yaml"), SECOND_CAPITAL_OVERLAY_YAML).unwrap();
    fs::write(dir.join("brainbrew.yaml"), CONFLICT_MANIFEST_YAML).unwrap();

    let output = run([
        "explain",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "conflict",
    ]);

    assert!(!output.status.success());
    assert!(stdout(&output).contains("2. patch.capital.second (second.yaml)"));
    assert!(stderr(&output).contains("conflicts with earlier overlay"));
}

#[test]
fn diff_can_emit_note_field_changes_as_overlay() {
    let dir = temp_dir("diff-as-overlay");
    let left = dir.join("left.yaml");
    let right = dir.join("right.yaml");
    fs::write(&left, SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        &right,
        SAMPLE_CANONICAL_YAML.replace("field.capital: Helsinki", "field.capital: Helsingfors"),
    )
    .unwrap();

    let output = run([
        "diff",
        left.to_str().unwrap(),
        right.to_str().unwrap(),
        "--as-overlay",
        "--id",
        "overlay.patch.capital",
        "--kind",
        "patch",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("id: overlay.patch.capital"));
    assert!(out.contains("kind: patch"));
    assert!(out.contains("field.capital:"));
    assert!(out.contains("value: Helsingfors"));
    assert!(out.contains("expected_base:\n          value: Helsinki"));
}

#[test]
fn diff_as_overlay_emits_tag_and_adapter_id_changes() {
    let dir = temp_dir("diff-as-overlay-tags");
    let left = dir.join("left.yaml");
    let right = dir.join("right.yaml");
    fs::write(&left, SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        &right,
        SAMPLE_CANONICAL_YAML
            .replace("      - Nordic", "      - Baltic")
            .replace("ug-finland-guid", "ug-finland-guid-v2"),
    )
    .unwrap();

    let output = run([
        "diff",
        left.to_str().unwrap(),
        right.to_str().unwrap(),
        "--as-overlay",
        "--id",
        "overlay.patch.tags",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("      Baltic:\n        intent: add"));
    assert!(
        out.contains(
            "      Nordic:\n        intent: remove\n        expected_base: entity_present"
        )
    );
    assert!(out.contains("      crowdanki:guid:\n        intent: replace"));
    assert!(out.contains("        value: ug-finland-guid-v2"));
}

#[test]
fn diff_as_overlay_emits_media_reference_changes() {
    let dir = temp_dir("diff-as-overlay-media");
    let left = dir.join("left.yaml");
    let right = dir.join("right.yaml");
    fs::write(&left, SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        &right,
        SAMPLE_CANONICAL_YAML.replace(
            "tombstones: []",
            "  media.flags-se-png:\n    path: flags/se.png\n    sha256: ''\ntombstones: []",
        ),
    )
    .unwrap();

    let output = run([
        "diff",
        left.to_str().unwrap(),
        right.to_str().unwrap(),
        "--as-overlay",
        "--id",
        "overlay.patch.media",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("media:\n  media.flags-se-png:\n    intent: add"));
    assert!(out.contains("    path: flags/se.png"));
}

#[test]
fn diff_as_overlay_emits_note_additions_and_removals() {
    let dir = temp_dir("diff-as-overlay-notes");
    let left = dir.join("left.yaml");
    let added = dir.join("added.yaml");
    let removed = dir.join("removed.yaml");
    fs::write(&left, SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        &added,
        SAMPLE_CANONICAL_YAML.replace(
            "media:\n  media.flags-fi-png:",
            "  note.sweden:\n    note_type_id: note-type.country\n    fields:\n      field.capital: Stockholm\n      field.country: Sweden\n      field.flag: '<img src=\"se.png\">'\n    tags:\n      - Europe\n      - Nordic\n    adapter_ids:\n      crowdanki:guid: ug-sweden-guid\nmedia:\n  media.flags-fi-png:",
        ),
    )
    .unwrap();
    fs::write(&removed, SAMPLE_WITHOUT_NOTES_CANONICAL_YAML).unwrap();

    let add_output = run([
        "diff",
        left.to_str().unwrap(),
        added.to_str().unwrap(),
        "--as-overlay",
        "--id",
        "overlay.patch.note-add",
    ]);
    assert!(
        add_output.status.success(),
        "stderr: {}",
        stderr(&add_output)
    );
    assert!(stdout(&add_output).contains("  note.sweden:\n    intent: add"));
    assert!(stdout(&add_output).contains("    note:\n      note_type_id: note-type.country"));

    let remove_output = run([
        "diff",
        left.to_str().unwrap(),
        removed.to_str().unwrap(),
        "--as-overlay",
        "--id",
        "overlay.patch.note-remove",
    ]);
    assert!(
        remove_output.status.success(),
        "stderr: {}",
        stderr(&remove_output)
    );
    assert!(
        stdout(&remove_output)
            .contains("  note.finland:\n    intent: remove\n    expected_base: entity_present")
    );
}

#[test]
fn diff_reports_json_changes_by_stable_path() {
    let dir = temp_dir("diff-json");
    let left = dir.join("left.yaml");
    let right = dir.join("right.yaml");
    fs::write(&left, SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        &right,
        SAMPLE_CANONICAL_YAML.replace("field.capital: Helsinki", "field.capital: Helsingfors"),
    )
    .unwrap();

    let output = run([
        "diff",
        left.to_str().unwrap(),
        right.to_str().unwrap(),
        "--json",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("\"path\": \"notes.note.finland.fields.field.capital\""));
    assert!(stdout(&output).contains("\"after\": \"Helsingfors\""));
}

#[test]
fn diff_reports_human_readable_before_and_after_values() {
    let dir = temp_dir("diff-human");
    let left = dir.join("left.yaml");
    let right = dir.join("right.yaml");
    fs::write(&left, SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        &right,
        SAMPLE_CANONICAL_YAML
            .replace("field.capital: Helsinki", "field.capital: Helsingfors")
            .replace("field.country: Finland", "field.country: Suomi"),
    )
    .unwrap();

    let output = run(["diff", left.to_str().unwrap(), right.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("2 semantic changes"));
    assert!(out.contains("~ notes.note.finland.fields.field.capital"));
    assert!(out.contains("- Helsinki"));
    assert!(out.contains("+ Helsingfors"));
    assert!(out.contains("~ notes.note.finland.fields.field.country"));
    assert!(out.contains("- Finland"));
    assert!(out.contains("+ Suomi"));
}

#[test]
fn file_includes_work_across_validate_compose_export_verify_diff_and_fmt() {
    let dir = temp_dir("file-includes-workflows");
    write_include_workspace(&dir);
    let manifest = dir.join("brainbrew.yaml");

    let validate = run([
        "validate",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "localized",
    ]);
    assert!(validate.status.success(), "stderr: {}", stderr(&validate));

    let resolved = dir.join("resolved.yaml");
    let compose = run([
        "compose",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "localized",
        "--out",
        resolved.to_str().unwrap(),
    ]);
    assert!(compose.status.success(), "stderr: {}", stderr(&compose));
    let resolved_source = fs::read_to_string(&resolved).unwrap();
    assert!(resolved_source.contains("Overlay description from Markdown."));
    assert!(resolved_source.contains("<section class=\"front\">{{Country}}</section>"));
    assert!(resolved_source.contains("font-family: sans-serif"));
    assert!(!resolved_source.contains("!include"));

    let export_dir = dir.join("crowdanki");
    let export = run([
        "export",
        "crowdanki",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "localized",
        "--out",
        export_dir.to_str().unwrap(),
    ]);
    assert!(export.status.success(), "stderr: {}", stderr(&export));
    assert!(
        fs::read_to_string(export_dir.join("deck.json"))
            .unwrap()
            .contains("<section class=\\\"front\\\">{{Country}}</section>")
    );

    let verify = run([
        "verify",
        "--manifest",
        manifest.to_str().unwrap(),
        "--all-targets",
    ]);
    assert!(verify.status.success(), "stderr: {}", stderr(&verify));

    let base_resolved = dir.join("base-resolved.yaml");
    let compose_base = run([
        "compose",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "base",
        "--out",
        base_resolved.to_str().unwrap(),
    ]);
    assert!(
        compose_base.status.success(),
        "stderr: {}",
        stderr(&compose_base)
    );
    let diff = run([
        "diff",
        dir.join("deck.yaml").to_str().unwrap(),
        base_resolved.to_str().unwrap(),
    ]);
    assert!(diff.status.success(), "stderr: {}", stderr(&diff));
    assert!(stdout(&diff).contains("no semantic changes"));

    let deck_for_fmt = dir.join("deck-to-format.yaml");
    fs::copy(dir.join("deck.yaml"), &deck_for_fmt).unwrap();
    let fmt = run(["fmt", deck_for_fmt.to_str().unwrap()]);
    assert!(fmt.status.success(), "stderr: {}", stderr(&fmt));
    let formatted = fs::read_to_string(deck_for_fmt).unwrap();
    assert!(formatted.contains("Base deck description."));
    assert!(formatted.contains("<section class=\"front\">{{Country}}</section>"));
    assert!(!formatted.contains("!include"));
}

#[test]
fn file_include_errors_name_referring_yaml_path_and_included_path() {
    let dir = temp_dir("file-include-errors");
    write_include_workspace(&dir);
    let deck_path = dir.join("deck.yaml");
    let deck = fs::read_to_string(&deck_path).unwrap();
    fs::write(
        &deck_path,
        deck.replace(
            "description: !include ./content/../content/base-description.md",
            "description: !include content/missing.md",
        ),
    )
    .unwrap();

    let output = run([
        "validate",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "base",
    ]);

    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("deck.description"), "stderr: {err}");
    assert!(err.contains("content/missing.md"), "stderr: {err}");
}

#[test]
fn file_includes_reject_package_root_escape_unless_safe_root_is_configured() {
    let root = temp_dir("file-include-roots");
    let package = root.join("package");
    let shared = root.join("shared");
    fs::create_dir_all(&package).unwrap();
    fs::create_dir_all(&shared).unwrap();
    write_include_workspace(&package);
    fs::write(
        shared.join("description.md"),
        "Shared safe-root description.\n",
    )
    .unwrap();
    let deck_path = package.join("deck.yaml");
    let deck = fs::read_to_string(&deck_path).unwrap();
    fs::write(
        &deck_path,
        deck.replace(
            "description: !include ./content/../content/base-description.md",
            "description: !include ../shared/description.md",
        ),
    )
    .unwrap();

    let rejected = run([
        "validate",
        "--manifest",
        package.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "base",
    ]);
    assert!(!rejected.status.success());
    let err = stderr(&rejected);
    assert!(err.contains("deck.description"), "stderr: {err}");
    assert!(err.contains("../shared/description.md"), "stderr: {err}");
    assert!(err.contains("escapes package root"), "stderr: {err}");

    let manifest_path = package.join("brainbrew.yaml");
    fs::write(
        &manifest_path,
        fs::read_to_string(&manifest_path).unwrap().replace(
            "base: deck.yaml\n",
            "base: deck.yaml\ninclude_roots:\n  - ../shared\n",
        ),
    )
    .unwrap();
    let accepted = run([
        "validate",
        "--manifest",
        manifest_path.to_str().unwrap(),
        "--target",
        "base",
    ]);
    assert!(accepted.status.success(), "stderr: {}", stderr(&accepted));
}

#[test]
fn file_include_cycles_are_reported_with_yaml_path_and_include_chain() {
    let dir = temp_dir("file-include-cycle");
    write_include_workspace(&dir);
    fs::write(dir.join("content/a.md"), "!include content/b.md\n").unwrap();
    fs::write(dir.join("content/b.md"), "!include content/a.md\n").unwrap();
    let deck_path = dir.join("deck.yaml");
    let deck = fs::read_to_string(&deck_path).unwrap();
    fs::write(
        &deck_path,
        deck.replace(
            "description: !include ./content/../content/base-description.md",
            "description: !include content/a.md",
        ),
    )
    .unwrap();

    let output = run([
        "validate",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "base",
    ]);

    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("deck.description"), "stderr: {err}");
    assert!(err.contains("content/a.md"), "stderr: {err}");
    assert!(err.contains("cyclic"), "stderr: {err}");
}

#[test]
fn file_includes_are_rejected_outside_scalar_content_fields() {
    let dir = temp_dir("file-include-invalid-target");
    fs::create_dir_all(dir.join("content")).unwrap();
    fs::write(dir.join("content/not-notes.yaml"), "{}\n").unwrap();
    let deck_path = dir.join("invalid.yaml");
    fs::write(
        &deck_path,
        r#"deck:
  id: deck.invalid-include
  name: Invalid Include
  description: Valid scalar description.
note_types: {}
notes: !include content/not-notes.yaml
media: {}
tombstones: []
"#,
    )
    .unwrap();

    let output = run(["validate", deck_path.to_str().unwrap()]);

    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("notes"), "stderr: {err}");
    assert!(err.contains("content/not-notes.yaml"), "stderr: {err}");
    assert!(err.contains("scalar content"), "stderr: {err}");
}

#[test]
fn export_and_import_crowdanki_deck_folder() {
    let dir = temp_dir("crowdanki-roundtrip");
    let deck_path = dir.join("deck.yaml");
    let export_dir = dir.join("crowdanki");
    let imported_path = dir.join("imported.yaml");
    fs::write(&deck_path, SAMPLE_CANONICAL_YAML).unwrap();

    let export_output = run([
        "export",
        "crowdanki",
        deck_path.to_str().unwrap(),
        "--out",
        export_dir.to_str().unwrap(),
    ]);
    assert!(
        export_output.status.success(),
        "stderr: {}",
        stderr(&export_output)
    );
    assert!(
        fs::read_to_string(export_dir.join("deck.json"))
            .unwrap()
            .contains("ug-finland-guid")
    );

    let import_output = run([
        "import",
        "crowdanki",
        export_dir.to_str().unwrap(),
        "--accept-suggested-ids",
        "--out",
        imported_path.to_str().unwrap(),
    ]);

    assert!(
        import_output.status.success(),
        "stderr: {}",
        stderr(&import_output)
    );
    assert!(
        fs::read_to_string(imported_path)
            .unwrap()
            .contains("id: deck.ultimate-geography")
    );
}

fn run<const N: usize>(args: [&str; N]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .output()
        .expect("command runs")
}

fn run_in_dir<const N: usize>(args: [&str; N], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("command runs")
}

fn run_with_env<const N: usize>(args: [&str; N], envs: &[(&str, &str)]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_brainbrew"));
    command.args(args);
    for (name, value) in envs {
        command.env(name, value);
    }
    command.output().expect("command runs")
}

fn run_with_stdin<const N: usize>(args: [&str; N], stdin: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("command spawns");
    child
        .stdin
        .as_mut()
        .expect("stdin is piped")
        .write_all(stdin.as_bytes())
        .expect("stdin writes");
    child.wait_with_output().expect("command runs")
}

fn run_with_cache<const N: usize>(args: [&str; N], cache: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .env("BRAINBREW_CACHE_DIR", cache)
        .output()
        .expect("command runs")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under crates/brain-brew-cli")
        .to_path_buf()
}

fn write_manifest_workspace(dir: &Path) {
    fs::write(dir.join("deck.yaml"), SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(dir.join("capital.yaml"), CAPITAL_OVERLAY_YAML).unwrap();
    fs::write(dir.join("noop.yaml"), NOOP_OVERLAY_YAML).unwrap();
    fs::write(dir.join("brainbrew.yaml"), MANIFEST_YAML).unwrap();
}

fn write_translation_workspace(dir: &Path) {
    let deck = SAMPLE_CANONICAL_YAML
        .replace("field.flag: '<img src=\"fi.png\">'", "field.flag: ''")
        .replace(
            "media:\n",
            r#"  note.sweden:
    note_type_id: note-type.country
    fields:
      field.capital: Stockholm
      field.country: Sweden
      field.flag: '<img src="se.png">'
    tags:
      - Europe
      - Nordic
    adapter_ids:
      crowdanki:guid: ug-sweden-guid
media:
"#,
        );
    fs::write(dir.join("deck.yaml"), deck).unwrap();
    fs::write(
        dir.join("da.yaml"),
        r#"id: overlay.translation.da
kind: translation
# translator note kept
translations:
  ignore_paths:
    - deck.*
    - note_types.*
    - notes.*.tags.*
    - notes.*.fields.field.flag
  direct:
    Finland: Finland
    Removed source: Fjernet
  contextual:
    notes.note.finland:
      Helsinki: Helsingfors
  target_additions:
    notes.note.finland.fields.field.flag: '<img src="fi-da.png">'
"#,
    )
    .unwrap();
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

fn write_structured_message_translation_workspace(dir: &Path) {
    fs::write(
        dir.join("deck.yaml"),
        r#"deck:
  id: deck.structured-message
  name: Structured Message Fixture
  description: Structured message translation fixture.
  adapter_ids: {}
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.country
      - field.capital
      - field.flag
      - field.flag-similarity
    fields:
      field.country:
        name: Country
      field.capital:
        name: Capital
      field.flag:
        name: Flag
      field.flag-similarity:
        name: Flag similarity
    card_template_order:
      - template.flag-country
    card_templates:
      template.flag-country:
        name: Flag - Country
        question_format: '{{Flag}}'
        answer_format: '{{Flag similarity}}'
        adapter_ids: {}
    styling: ''
    adapter_ids: {}
notes:
  note.finland:
    note_type_id: note-type.country
    fields:
      field.capital: Helsinki
      field.country: Finland
      field.flag: '<img src="fi.png">'
      field.flag-similarity:
        message:
          - ref: notes.note.iceland.fields.field.country
          - literal: ' ('
          - text: blue background with a white cross
          - literal: '), '
          - ref: notes.note.norway.fields.field.country
          - literal: ' ('
          - text: red background with a blue cross
          - literal: ')'
    tags: []
    adapter_ids: {}
  note.iceland:
    note_type_id: note-type.country
    fields:
      field.capital: Reykjavik
      field.country: Iceland
      field.flag: '<img src="is.png">'
      field.flag-similarity: ''
    tags: []
    adapter_ids: {}
  note.norway:
    note_type_id: note-type.country
    fields:
      field.capital: Oslo
      field.country: Norway
      field.flag: '<img src="no.png">'
      field.flag-similarity: ''
    tags: []
    adapter_ids: {}
media: {}
tombstones: []
"#,
    )
    .unwrap();
    fs::write(
        dir.join("nb.yaml"),
        r#"id: overlay.translation.nb
kind: translation
translations:
  ignore_paths:
    - deck.*
    - note_types.*
    - notes.*.fields.field.country
    - notes.*.fields.field.capital
    - notes.*.fields.field.flag
    - notes.*.tags.*
  direct:
    Iceland: Island
    Norway: Norge
    red background with a blue cross: rød bakgrunn med blått kors
  contextual:
    notes.note.finland:
      blue background with a white cross: blå bakgrunn med hvitt kors
"#,
    )
    .unwrap();
    fs::write(
        dir.join("brainbrew.yaml"),
        r#"base: deck.yaml
overlays:
  overlay.translation.nb:
    file: nb.yaml
    kind: translation
targets:
  nb-standard:
    overlays:
      - overlay.translation.nb
"#,
    )
    .unwrap();
}

fn write_include_workspace(dir: &Path) {
    fs::create_dir_all(dir.join("content")).unwrap();
    fs::create_dir_all(dir.join("templates")).unwrap();
    fs::create_dir_all(dir.join("styles")).unwrap();
    fs::create_dir_all(dir.join("overlays")).unwrap();
    fs::write(
        dir.join("content/base-description.md"),
        "Base deck description.\nWritten in Markdown.\n",
    )
    .unwrap();
    fs::write(
        dir.join("content/overlay-description.md"),
        "Overlay description from Markdown.\n",
    )
    .unwrap();
    fs::write(
        dir.join("templates/front.html"),
        "<section class=\"front\">{{Country}}</section>\n",
    )
    .unwrap();
    fs::write(
        dir.join("templates/back.html"),
        "{{FrontSide}}\n<hr id=\"answer\">\n<section class=\"back\">{{Capital}}</section>\n",
    )
    .unwrap();
    fs::write(
        dir.join("styles/card.css"),
        ".card {\n  font-family: sans-serif;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("deck.yaml"),
        r#"deck:
  id: deck.include-fixture
  name: Include Fixture
  description: !include ./content/../content/base-description.md
  adapter_ids:
    crowdanki:uuid: include-fixture-deck-uuid
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
      - template.country-capital
    card_templates:
      template.country-capital:
        name: Country - Capital
        question_format: !include templates/front.html
        answer_format: !include templates/back.html
        adapter_ids: {}
    styling: !include styles/card.css
    adapter_ids:
      crowdanki:uuid: include-fixture-note-type-uuid
notes:
  note.finland:
    note_type_id: note-type.country
    fields:
      field.capital: Helsinki
      field.country: Finland
    tags:
      - Europe
    adapter_ids:
      crowdanki:guid: include-fixture-note-guid
media: {}
tombstones: []
"#,
    )
    .unwrap();
    fs::write(
        dir.join("overlays/description.yaml"),
        r#"id: overlay.patch.description
kind: patch
deck:
  description:
    intent: replace
    value: !include content/overlay-description.md
    expected_base:
      value: |
        Base deck description.
        Written in Markdown.
"#,
    )
    .unwrap();
    fs::write(
        dir.join("brainbrew.yaml"),
        r#"base: deck.yaml
overlays:
  overlay.patch.description:
    file: overlays/description.yaml
    kind: patch
targets:
  base:
    overlays: []
  localized:
    overlays:
      - overlay.patch.description
"#,
    )
    .unwrap();
}

fn write_tar_gz(path: &Path, root_name: &str, source_dir: &Path) {
    let file = fs::File::create(path).unwrap();
    let encoder = GzEncoder::new(file, Compression::default());
    let mut archive = Builder::new(encoder);
    archive.append_dir_all(root_name, source_dir).unwrap();
    let encoder = archive.into_inner().unwrap();
    encoder.finish().unwrap();
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

const CAPITAL_OVERLAY_YAML: &str = r#"id: overlay.patch.capital
kind: patch
notes:
  note.finland:
    intent: merge
    fields:
      field.capital:
        intent: replace
        value: Helsingfors
        expected_base:
          value: Helsinki
"#;

const MESSY_OVERLAY_YAML: &str = r#"kind: patch
id: overlay.patch.capital
notes:
  note.finland:
    fields:
      field.capital:
        expected_base:
          value: Helsinki
        value: Helsingfors
        intent: replace
    intent: merge
"#;

const NOOP_OVERLAY_YAML: &str = r#"id: overlay.noop
kind: patch
"#;

const SECOND_CAPITAL_OVERLAY_YAML: &str = r#"id: overlay.patch.capital.second
kind: patch
notes:
  note.finland:
    intent: merge
    fields:
      field.capital:
        intent: replace
        value: Helsinki City
        expected_base:
          value: Helsinki
"#;

const MANIFEST_YAML: &str = r#"base: deck.yaml
overlays:
  noop.after-capital:
    file: noop.yaml
    kind: patch
    depends_on:
      - patch.capital
  patch.capital:
    file: capital.yaml
    kind: patch
targets:
  patched-via-dependency:
    overlays:
      - noop.after-capital
"#;

const MANIFEST_WITH_EXPORTS_YAML: &str = r#"base: deck.yaml
overlays:
  noop.after-capital:
    file: noop.yaml
    kind: patch
    depends_on:
      - patch.capital
  patch.capital:
    file: capital.yaml
    kind: patch
targets:
  patched-via-dependency:
    overlays:
      - noop.after-capital
    exports:
      crowdanki:
        out: configured-crowdanki
        golden: goldens/patched/deck.json
"#;

const MANIFEST_WITH_PACKAGE_YAML: &str = r#"package:
  id: anki-geo.ultimate-geography
  version: 0.1.0
  compatible_base_versions:
    - '>=0.1,<0.2'
  depends_on:
    - anki-geo.shared-geography
base: deck.yaml
overlays:
  noop.after-capital:
    file: noop.yaml
    kind: patch
    depends_on:
      - patch.capital
  patch.capital:
    file: capital.yaml
    kind: patch
targets:
  patched-via-dependency:
    overlays:
      - noop.after-capital
"#;

const CONFLICT_MANIFEST_YAML: &str = r#"base: deck.yaml
overlays:
  patch.capital:
    file: capital.yaml
    kind: patch
  patch.capital.second:
    file: second.yaml
    kind: patch
targets:
  conflict:
    overlays:
      - patch.capital
      - patch.capital.second
"#;

const SIMPLE_MEDIA_MANIFEST_YAML: &str = r#"base: deck.yaml
overlays: {}
targets:
  base:
    overlays: []
"#;

const MESSY_MANIFEST_YAML: &str = r#"targets:
  patched-via-dependency:
    overlays: [noop.after-capital]
overlays:
  patch.capital:
    kind: patch
    file: capital.yaml
  noop.after-capital:
    depends_on: [patch.capital]
    kind: patch
    file: noop.yaml
base: deck.yaml
"#;

const LOCK_YAML: &str = r#"version: 1
packages:
  anki-geo.ultimate-geography:
    manifest: brainbrew.yaml
    package:
      version: 0.1.0
    original:
      type: git
      url: https://github.com/anki-geo/ultimate-geography.git
      ref: main
    locked:
      type: git
      url: https://github.com/anki-geo/ultimate-geography.git
      rev: ccf150a1b21e
      nar_hash: sha256-example
"#;

const MESSY_LOCK_YAML: &str = r#"packages:
  anki-geo.ultimate-geography:
    locked:
      nar_hash: sha256-example
      rev: ccf150a1b21e
      url: https://github.com/anki-geo/ultimate-geography.git
      type: git
    original:
      ref: main
      url: https://github.com/anki-geo/ultimate-geography.git
      type: git
    package:
      version: 0.1.0
    manifest: brainbrew.yaml
version: 1
"#;

const MESSY_CANONICAL_YAML: &str = r#"deck:
  description: A geography deck fixture.
  id: deck.ultimate-geography
  name: Ultimate Geography
  adapter_ids:
    crowdanki:uuid: 43c5ba66-9a65-11e8-90c9-a0481cc15658
note_types:
  note-type.country:
    adapter_ids:
      crowdanki:uuid: aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa
    name: Country
    styling: |
      .card { font-family: sans-serif; }
    field_order: [field.country, field.capital, field.flag]
    fields:
      field.flag: { name: Flag }
      field.capital: { name: Capital }
      field.country: { name: Country }
    card_template_order: [template.country-capital]
    card_templates:
      template.country-capital:
        adapter_ids: {}
        name: Country - Capital
        question_format: '{{Country}}'
        answer_format: '{{FrontSide}}<hr id=answer>{{Capital}}'
notes:
  note.finland:
    adapter_ids:
      crowdanki:guid: ug-finland-guid
    fields:
      field.flag: '<img src="fi.png">'
      field.capital: Helsinki
      field.country: Finland
    note_type_id: note-type.country
    tags: [Europe, Nordic]
media:
  media.flags-fi-png:
    path: flags/fi.png
    sha256: ''
tombstones: []
"#;

const SAMPLE_WITHOUT_NOTES_CANONICAL_YAML: &str = r#"deck:
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
notes: {}
media:
  media.flags-fi-png:
    path: flags/fi.png
    sha256: ''
tombstones: []
"#;

const MEDIA_CANONICAL_YAML: &str = r#"deck:
  id: deck.media-fixture
  name: Media Fixture
  description: A deck with media.
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
      field.flag: '<img src="flags/fi.png">'
    tags:
      - Media
    adapter_ids:
      crowdanki:guid: media-fi-guid
media:
  media.flags-fi-png:
    path: flags/fi.png
    sha256: 14873f4faae48052921f9272d948a369f775b2406e57a9b8d55fb94452b73948
tombstones: []
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
      field.flag: '<img src="fi.png">'
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
