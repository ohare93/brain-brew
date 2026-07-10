use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use brain_brew_formats::media::sha256_hex;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .output()
        .expect("brainbrew runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn hashless_ug_sd_tasks_are_explicitly_non_release() {
    let tasks_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.sd/tasks.yaml");
    let document: serde_yaml::Value =
        serde_yaml::from_str(&fs::read_to_string(tasks_path).unwrap()).unwrap();

    for task_name in ["run/ug/export/en-standard", "run/ug/verify/all"] {
        let task = &document["tasks"][task_name];
        let run = task["run"].as_str().unwrap();
        let description = task["desc"].as_str().unwrap();
        assert!(
            run.contains("--media-mode reference-only"),
            "{task_name} must explicitly select reference-only media verification"
        );
        assert!(
            description.contains("hashless structural fixture")
                && description.contains("not release or byte-integrity evidence"),
            "{task_name} must identify its output as non-release structural evidence"
        );
    }
}

fn write_workspace(root: &Path, path: &str, hash: &str) -> PathBuf {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("deck.yaml"),
        format!(
            r#"deck:
  id: deck.media
  name: Media
  description: Media integrity test.
  adapter_ids:
    crowdanki:uuid: 43c5ba66-9a65-11e8-90c9-a0481cc15658
note_types:
  note-type.media:
    name: Media
    field_order:
      - field.front
    fields:
      field.front:
        name: Front
    card_template_order:
      - template.media
    card_templates:
      template.media:
        name: Media
        question_format: '{{{{Front}}}}'
        answer_format: '{{{{FrontSide}}}}'
        adapter_ids: {{}}
    styling: ''
    adapter_ids:
      crowdanki:uuid: aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa
notes:
  note.media:
    note_type_id: note-type.media
    fields:
      field.front: !image media.asset
    tags: []
    adapter_ids:
      crowdanki:guid: media-guid
media:
  media.asset:
    path: '{}'
    sha256: '{}'
tombstones: []
"#,
            path.replace('\'', "''"),
            hash.replace('\'', "''")
        ),
    )
    .unwrap();
    let manifest = root.join("brainbrew.yaml");
    fs::write(
        &manifest,
        "package:\n  id: example.media\n  version: 1.0.0\nbase: deck.yaml\noverlays: {}\ntargets:\n  release:\n    overlays: []\n",
    )
    .unwrap();
    for source in [root.join("deck.yaml"), manifest.clone()] {
        let formatted = run(&["fmt", source.to_str().unwrap()]);
        assert!(formatted.status.success(), "{}", stderr(&formatted));
    }
    manifest
}

fn verify(manifest: &Path, extra: &[&str]) -> Output {
    let mut args = vec![
        "verify",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "release",
    ];
    args.extend_from_slice(extra);
    run(&args)
}

#[test]
fn targets_without_media_remain_strictly_verifiable_without_roots() {
    let temp = tempfile::tempdir().unwrap();
    let manifest = write_workspace(temp.path(), "unused.png", "");
    let deck = fs::read_to_string(temp.path().join("deck.yaml"))
        .unwrap()
        .replace("field.front: !image media.asset", "field.front: No media")
        .replace(
            "media:\n  media.asset:\n    path: unused.png\n    sha256: ''",
            "media: {}",
        );
    fs::write(temp.path().join("deck.yaml"), deck).unwrap();
    let formatted = run(&["fmt", temp.path().join("deck.yaml").to_str().unwrap()]);
    assert!(formatted.status.success(), "{}", stderr(&formatted));

    let result = verify(&manifest, &[]);
    assert!(result.status.success(), "{}", stderr(&result));
    assert!(stdout(&result).contains("media verification: strict"));
}

#[test]
fn strict_default_requires_hash_root_present_bytes_and_match_then_succeeds() {
    let temp = tempfile::tempdir().unwrap();
    let bytes = b"verified media bytes";
    let hash = sha256_hex(bytes);
    let manifest = write_workspace(temp.path(), "assets/flag.png", &hash);

    let missing_root = verify(&manifest, &[]);
    assert!(!missing_root.status.success());
    assert!(stderr(&missing_root).contains("missing media root"));

    let media_root = temp.path().join("media");
    fs::create_dir_all(&media_root).unwrap();
    let missing_bytes = verify(&manifest, &["--media-root", media_root.to_str().unwrap()]);
    assert!(!missing_bytes.status.success());
    assert!(stderr(&missing_bytes).contains("No such file"));

    fs::create_dir_all(media_root.join("assets")).unwrap();
    fs::write(media_root.join("assets/flag.png"), b"wrong").unwrap();
    let mismatch = verify(&manifest, &["--media-root", media_root.to_str().unwrap()]);
    assert!(!mismatch.status.success());
    assert!(stderr(&mismatch).contains("sha256 mismatch"));

    fs::write(media_root.join("assets/flag.png"), bytes).unwrap();
    let success = verify(&manifest, &["--media-root", media_root.to_str().unwrap()]);
    assert!(success.status.success(), "{}", stderr(&success));
    assert!(stdout(&success).contains("media verification: strict"));
}

#[test]
fn strict_rejects_missing_or_noncanonical_hash_and_reference_only_never_hides_bad_syntax() {
    let temp = tempfile::tempdir().unwrap();
    let hashless = write_workspace(temp.path(), "flag.png", "");
    let strict = verify(&hashless, &[]);
    assert!(!strict.status.success());
    assert!(stderr(&strict).contains("empty sha256"));

    let malformed_root = tempfile::tempdir().unwrap();
    let malformed = write_workspace(malformed_root.path(), "flag.png", "ABC123");
    let reference_only = verify(&malformed, &["--media-mode", "reference-only"]);
    assert!(!reference_only.status.success());
    assert!(stderr(&reference_only).contains("64 lowercase hexadecimal"));
}

#[test]
fn explicit_reference_only_reports_not_release_ready_in_human_and_json_output() {
    let temp = tempfile::tempdir().unwrap();
    let manifest = write_workspace(temp.path(), "flag.png", "");

    let human = verify(&manifest, &["--media-mode", "reference-only"]);
    assert!(human.status.success(), "{}", stderr(&human));
    assert!(stderr(&human).contains("NOT RELEASE-READY"));
    assert!(stdout(&human).contains("reference_only (NOT RELEASE-READY)"));

    let json = verify(&manifest, &["--media-mode", "reference-only", "--json"]);
    assert!(json.status.success(), "{}", stderr(&json));
    assert!(json.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(value["media"]["mode"], "reference_only");
    assert_eq!(value["media"]["release_ready"], false);
    assert!(
        value["warnings"][0]
            .as_str()
            .unwrap()
            .contains("NOT RELEASE-READY")
    );

    let export_dir = temp.path().join("reference-export");
    let export = run(&[
        "export",
        "crowdanki",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "release",
        "--media-mode",
        "reference-only",
        "--json",
        "--out",
        export_dir.to_str().unwrap(),
    ]);
    assert!(export.status.success(), "{}", stderr(&export));
    assert!(export.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&export.stdout).unwrap();
    assert_eq!(value["media"]["mode"], "reference_only");
    assert_eq!(value["media"]["release_ready"], false);
    assert_eq!(value["media"]["assets_copied"], 0);
}

#[test]
fn failed_strict_export_preserves_prior_tree_and_leaves_no_stage() {
    let temp = tempfile::tempdir().unwrap();
    let manifest = write_workspace(temp.path(), "flag.png", &sha256_hex(b"expected"));
    let media_root = temp.path().join("media");
    fs::create_dir_all(&media_root).unwrap();
    fs::write(media_root.join("flag.png"), b"wrong").unwrap();
    let output = temp.path().join("out");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("prior.txt"), b"prior complete tree").unwrap();

    let result = run(&[
        "export",
        "crowdanki",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "release",
        "--media-root",
        media_root.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
        "--force",
    ]);

    assert!(!result.status.success());
    assert_eq!(
        fs::read(output.join("prior.txt")).unwrap(),
        b"prior complete tree"
    );
    assert!(!output.join("deck.json").exists());
    let siblings = fs::read_dir(temp.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(
        !siblings.iter().any(|name| name.contains(".stage")),
        "{siblings:?}"
    );
}

#[test]
fn reference_only_export_still_validates_references_before_publication() {
    let temp = tempfile::tempdir().unwrap();
    let manifest = write_workspace(temp.path(), "declared.png", "");
    let deck = fs::read_to_string(temp.path().join("deck.yaml"))
        .unwrap()
        .replace(
            "field.front: !image media.asset",
            "field.front: '<img src=\"undeclared.png\" />'",
        );
    fs::write(temp.path().join("deck.yaml"), deck).unwrap();
    let formatted = run(&["fmt", temp.path().join("deck.yaml").to_str().unwrap()]);
    assert!(formatted.status.success(), "{}", stderr(&formatted));
    let output = temp.path().join("out");

    let result = run(&[
        "export",
        "crowdanki",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "release",
        "--media-mode",
        "reference-only",
        "--out",
        output.to_str().unwrap(),
    ]);

    assert!(!result.status.success());
    assert!(stderr(&result).contains("used but not declared"));
    assert!(!output.exists());
}

#[cfg(unix)]
#[test]
fn structured_hostile_filename_is_encoded_in_html_but_preserved_for_crowdanki_and_copy() {
    let temp = tempfile::tempdir().unwrap();
    let path = "images/旗 & quote\" #1?.svg";
    let bytes = b"hostile filename bytes";
    let manifest = write_workspace(temp.path(), path, &sha256_hex(bytes));
    let media_root = temp.path().join("media");
    fs::create_dir_all(media_root.join("images")).unwrap();
    fs::write(media_root.join(path), bytes).unwrap();
    let output = temp.path().join("out");

    let result = run(&[
        "export",
        "crowdanki",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "release",
        "--media-root",
        media_root.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
    ]);
    assert!(result.status.success(), "{}", stderr(&result));
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output.join("deck.json")).unwrap()).unwrap();
    assert_eq!(json["media_files"][0], path);
    assert_eq!(
        json["notes"][0]["fields"][0],
        r#"<img src="images/%E6%97%97%20%26%20quote%22%20%231%3F.svg" />"#
    );
    assert_eq!(fs::read(output.join("media").join(path)).unwrap(), bytes);
}
