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

fn write_workspace(root: &Path) -> PathBuf {
    fs::create_dir_all(root.join("src/media")).unwrap();
    fs::create_dir_all(root.join("media")).unwrap();
    fs::write(root.join("src/media/runtime.js"), b"console.log('one');\n").unwrap();
    fs::write(root.join("media/flag.svg"), b"flag bytes").unwrap();
    fs::write(
        root.join("deck.yaml"),
        r#"deck:
  id: deck.source-media
  name: Source media
  description: Source media test.
  adapter_ids:
    crowdanki:uuid: 43c5ba66-9a65-11e8-90c9-a0481cc15658
note_types:
  note-type.basic:
    name: Basic
    field_order:
      - field.front
    fields:
      field.front:
        name: Front
    card_template_order:
      - template.basic
    card_templates:
      template.basic:
        name: Basic
        question_format: '<img src="flag.svg" />'
        answer_format: '<script src="runtime.js"></script>'
        adapter_ids: {}
    styling: ''
    adapter_ids:
      crowdanki:uuid: aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa
notes:
  note.one:
    note_type_id: note-type.basic
    fields:
      field.front: Front
    tags: []
    adapter_ids:
      crowdanki:guid: source-media-note
media:
  media.flag:
    path: flag.svg
    sha256: ''
tombstones: []
"#,
    )
    .unwrap();
    fs::write(
        root.join("runtime.yaml"),
        r#"id: overlay.runtime
kind: extension
media:
  media.runtime:
    intent: add
    path: runtime.js
    source: src/media/runtime.js
    sha256: ''
"#,
    )
    .unwrap();
    let manifest = root.join("brainbrew.yaml");
    fs::write(
        &manifest,
        "package:\n  id: example.source-media\n  version: 1.0.0\nbase: deck.yaml\noverlays:\n  overlay.runtime:\n    file: runtime.yaml\n    kind: extension\ntargets:\n  mixed:\n    overlays:\n      - overlay.runtime\n",
    )
    .unwrap();
    manifest
}

#[test]
fn source_backed_base_and_overlay_need_no_media_root() {
    let temp = tempfile::tempdir().unwrap();
    let manifest = write_workspace(temp.path());
    fs::write(temp.path().join("src/media/flag.svg"), b"source flag").unwrap();
    fs::remove_file(temp.path().join("media/flag.svg")).unwrap();
    let deck = fs::read_to_string(temp.path().join("deck.yaml"))
        .unwrap()
        .replace(
            "    sha256: ''",
            "    source: src/media/flag.svg\n    sha256: ''",
        );
    fs::write(temp.path().join("deck.yaml"), deck).unwrap();

    let hash = run(&[
        "media",
        "hash",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "mixed",
    ]);
    assert!(hash.status.success(), "{}", stderr(&hash));
    let formatted = run(&["fmt", temp.path().join("deck.yaml").to_str().unwrap()]);
    assert!(formatted.status.success(), "{}", stderr(&formatted));
    assert!(
        fs::read_to_string(temp.path().join("deck.yaml"))
            .unwrap()
            .contains("source: src/media/flag.svg\n")
    );

    let verify = run(&[
        "verify",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "mixed",
    ]);
    assert!(verify.status.success(), "{}", stderr(&verify));
    let output = temp.path().join("source-only-export");
    let export = run(&[
        "export",
        "crowdanki",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "mixed",
        "--out",
        output.to_str().unwrap(),
    ]);
    assert!(export.status.success(), "{}", stderr(&export));
    assert_eq!(
        fs::read(output.join("media/flag.svg")).unwrap(),
        b"source flag"
    );
    assert_eq!(
        fs::read(output.join("media/runtime.js")).unwrap(),
        b"console.log('one');\n"
    );
}

#[test]
fn source_backed_overlay_hashes_verifies_and_exports_with_hashed_media() {
    let temp = tempfile::tempdir().unwrap();
    let manifest = write_workspace(temp.path());
    let media_root = temp.path().join("media");

    let hash = run(&[
        "media",
        "hash",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "mixed",
        "--media-root",
        media_root.to_str().unwrap(),
    ]);
    assert!(hash.status.success(), "{}", stderr(&hash));
    let explain = run(&[
        "explain",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "mixed",
        "--json",
    ]);
    assert!(explain.status.success(), "{}", stderr(&explain));
    let explained: serde_json::Value = serde_json::from_str(&stdout(&explain)).unwrap();
    assert!(
        explained["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| {
                source["kind"] == "media_asset"
                    && source["path"]
                        .as_str()
                        .unwrap()
                        .ends_with("src/media/runtime.js")
                    && source["sha256"].as_str().unwrap().len() == 64
            })
    );
    let overlay = fs::read_to_string(temp.path().join("runtime.yaml")).unwrap();
    assert!(
        overlay.contains("source: src/media/runtime.js\n"),
        "{overlay}"
    );
    assert!(
        overlay.contains(&format!("sha256: {}", sha256_hex(b"console.log('one');\n"))),
        "{overlay}"
    );

    let verify = run(&[
        "verify",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "mixed",
        "--media-root",
        media_root.to_str().unwrap(),
    ]);
    assert!(verify.status.success(), "{}", stderr(&verify));

    let output = temp.path().join("export");
    let export = run(&[
        "export",
        "crowdanki",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "mixed",
        "--media-root",
        media_root.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
    ]);
    assert!(export.status.success(), "{}", stderr(&export));
    assert_eq!(
        fs::read(output.join("media/runtime.js")).unwrap(),
        b"console.log('one');\n"
    );
    assert_eq!(
        fs::read(output.join("media/flag.svg")).unwrap(),
        b"flag bytes"
    );

    fs::write(
        temp.path().join("src/media/runtime.js"),
        b"console.log('two');\n",
    )
    .unwrap();
    let stale = run(&[
        "verify",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "mixed",
        "--media-root",
        media_root.to_str().unwrap(),
    ]);
    assert!(!stale.status.success());
    assert!(stderr(&stale).contains("sha256 mismatch"));

    let refresh = run(&[
        "media",
        "hash",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "mixed",
        "--media-root",
        media_root.to_str().unwrap(),
    ]);
    assert!(refresh.status.success(), "{}", stderr(&refresh));
    let refreshed = fs::read_to_string(temp.path().join("runtime.yaml")).unwrap();
    assert!(refreshed.contains("source: src/media/runtime.js\n"));
    assert!(refreshed.contains(&sha256_hex(b"console.log('two');\n")));

    let second_output = temp.path().join("second-export");
    let export = run(&[
        "export",
        "crowdanki",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "mixed",
        "--media-root",
        media_root.to_str().unwrap(),
        "--out",
        second_output.to_str().unwrap(),
    ]);
    assert!(export.status.success(), "{}", stderr(&export));
    assert_eq!(
        fs::read(second_output.join("media/runtime.js")).unwrap(),
        b"console.log('two');\n"
    );
}

#[test]
fn unsafe_or_missing_media_sources_fail_before_export_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let manifest = write_workspace(temp.path());
    let output = temp.path().join("out");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("prior.txt"), b"prior").unwrap();
    let overlay_path = temp.path().join("runtime.yaml");

    for source in ["../outside.js", "src/media/missing.js"] {
        let overlay = fs::read_to_string(&overlay_path)
            .unwrap()
            .replace("src/media/runtime.js", source);
        fs::write(&overlay_path, overlay).unwrap();
        let export = run(&[
            "export",
            "crowdanki",
            "--manifest",
            manifest.to_str().unwrap(),
            "--target",
            "mixed",
            "--media-root",
            temp.path().join("media").to_str().unwrap(),
            "--out",
            output.to_str().unwrap(),
            "--force",
        ]);
        assert!(!export.status.success());
        assert_eq!(fs::read(output.join("prior.txt")).unwrap(), b"prior");
        assert!(!output.join("deck.json").exists());
        fs::write(
            &overlay_path,
            fs::read_to_string(&overlay_path)
                .unwrap()
                .replace(source, "src/media/runtime.js"),
        )
        .unwrap();
    }
}

#[cfg(unix)]
#[test]
fn escaping_media_source_symlinks_fail_before_export_mutation() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let manifest = write_workspace(temp.path());
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("outside.js"), b"outside").unwrap();
    symlink(
        outside.path().join("outside.js"),
        temp.path().join("src/media/link.js"),
    )
    .unwrap();
    let overlay_path = temp.path().join("runtime.yaml");
    let overlay = fs::read_to_string(&overlay_path)
        .unwrap()
        .replace("src/media/runtime.js", "src/media/link.js");
    fs::write(&overlay_path, overlay).unwrap();
    let output = temp.path().join("out");

    let export = run(&[
        "export",
        "crowdanki",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "mixed",
        "--media-root",
        temp.path().join("media").to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
    ]);
    assert!(!export.status.success());
    assert!(stderr(&export).contains("escapes"));
    assert!(!output.exists());
}
