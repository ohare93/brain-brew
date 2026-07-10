use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .output()
        .expect("brainbrew runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn write_package(root: &Path, id: &str, dependencies: &[&str], manifest_body: &str) -> PathBuf {
    fs::create_dir_all(root).unwrap();
    fs::write(root.join("deck.yaml"), DECK).unwrap();
    let depends = if dependencies.is_empty() {
        String::new()
    } else {
        format!(
            "  depends_on:\n{}",
            dependencies
                .iter()
                .map(|dependency| format!("    - {dependency}\n"))
                .collect::<String>()
        )
    };
    fs::write(
        root.join("brainbrew.yaml"),
        format!(
            "package:\n  id: {id}\n  version: 1.0.0\n{depends}base: deck.yaml\noverlays: {{}}\ntargets:\n{manifest_body}"
        ),
    )
    .unwrap();
    root.join("brainbrew.yaml")
}

#[test]
fn qualified_targets_have_route_and_json_parity_and_unqualified_ambiguity_fails() {
    let temp = tempfile::tempdir().unwrap();
    let upstream = temp.path().join("upstream");
    let root = temp.path().join("root");
    let upstream_manifest = write_package(
        &upstream,
        "example.up",
        &[],
        "  common:\n    overlays: []\n",
    );
    fs::write(
        upstream.join("patch.yaml"),
        "id: overlay.patch\nkind: patch\ndeck:\n  name:\n    intent: replace\n    value: Upstream Patched\n    expected_base:\n      value: Example\n",
    )
    .unwrap();
    fs::write(
        &upstream_manifest,
        "package:\n  id: example.up\n  version: 1.0.0\nbase: deck.yaml\noverlays:\n  overlay.patch:\n    file: patch.yaml\n    kind: patch\ntargets:\n  common:\n    overlays:\n      - overlay.patch\n",
    )
    .unwrap();
    let root_manifest = write_package(
        &root,
        "example.root",
        &["example.up@1.0.0"],
        "  common:\n    overlays: []\n  inherited:\n    extends: example.up:common\n    overlays: []\n",
    );
    let include = upstream_manifest.to_str().unwrap();
    let manifest = root_manifest.to_str().unwrap();
    for source in [manifest, include] {
        let formatted = run(&["fmt", source]);
        assert!(formatted.status.success(), "{}", stderr(&formatted));
    }

    let targets = run(&[
        "targets",
        "--manifest",
        manifest,
        "--include",
        include,
        "--json",
    ]);
    assert!(targets.status.success(), "{}", stderr(&targets));
    let json: serde_json::Value = serde_json::from_slice(&targets.stdout).unwrap();
    let inherited = json["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|target| target["qualified_name"] == "example.root:inherited")
        .unwrap();
    assert_eq!(
        inherited["overlays"][0]["qualified_id"],
        "example.up:overlay.patch"
    );

    let ambiguous = run(&[
        "compose",
        "--manifest",
        manifest,
        "--include",
        include,
        "--target",
        "common",
    ]);
    assert!(!ambiguous.status.success());
    assert!(stderr(&ambiguous).contains("ambiguous unqualified target"));

    let out = temp.path().join("resolved.yaml");
    for command in ["compose", "validate", "explain"] {
        let mut args = vec![
            command,
            "--manifest",
            manifest,
            "--include",
            include,
            "--target",
            "example.up:common",
        ];
        if command == "compose" {
            args.extend(["--out", out.to_str().unwrap()]);
        }
        let output = run(&args);
        assert!(output.status.success(), "{command}: {}", stderr(&output));
    }
    assert!(
        fs::read_to_string(&out)
            .unwrap()
            .contains("Upstream Patched")
    );

    let verify = run(&[
        "verify",
        "--manifest",
        manifest,
        "--include",
        include,
        "--target",
        "example.up:common",
    ]);
    assert!(verify.status.success(), "{}", stderr(&verify));

    let translations = run(&[
        "translations",
        "--manifest",
        manifest,
        "--include",
        include,
        "--target",
        "example.up:common",
    ]);
    assert!(translations.status.success(), "{}", stderr(&translations));

    let media = run(&[
        "media",
        "images-to-refs",
        "--manifest",
        manifest,
        "--include",
        include,
        "--target",
        "example.up:common",
    ]);
    assert!(media.status.success(), "{}", stderr(&media));

    let export_dir = temp.path().join("export");
    let export = run(&[
        "export",
        "crowdanki",
        "--manifest",
        manifest,
        "--include",
        include,
        "--target",
        "example.up:common",
        "--out",
        export_dir.to_str().unwrap(),
    ]);
    assert!(export.status.success(), "{}", stderr(&export));
    assert!(export_dir.join("deck.json").exists());
}

#[test]
fn registry_rejects_duplicate_identities_and_cross_package_target_cycles() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    let first_manifest = write_package(&first, "example.same", &[], "  first:\n    overlays: []\n");
    let second_manifest = write_package(
        &second,
        "example.same",
        &[],
        "  second:\n    overlays: []\n",
    );
    let duplicate = run(&[
        "targets",
        "--manifest",
        first_manifest.to_str().unwrap(),
        "--include",
        second_manifest.to_str().unwrap(),
    ]);
    assert!(!duplicate.status.success());
    assert!(stderr(&duplicate).contains("duplicate package identity"));

    fs::write(
        &first_manifest,
        fs::read_to_string(&first_manifest)
            .unwrap()
            .replace("example.same", "example.first")
            .replace(
                "  first:\n    overlays: []",
                "  first:\n    extends: example.second:second\n    overlays: []",
            ),
    )
    .unwrap();
    fs::write(
        &second_manifest,
        fs::read_to_string(&second_manifest)
            .unwrap()
            .replace("example.same", "example.second")
            .replace(
                "  second:\n    overlays: []",
                "  second:\n    extends: example.first:first\n    overlays: []",
            ),
    )
    .unwrap();
    let cycle = run(&[
        "targets",
        "--manifest",
        first_manifest.to_str().unwrap(),
        "--include",
        second_manifest.to_str().unwrap(),
    ]);
    assert!(!cycle.status.success());
    let error = stderr(&cycle);
    assert!(
        error.contains("manifest target dependency cycle"),
        "{error}"
    );
    assert!(error.contains("example.first:first"), "{error}");
    assert!(error.contains("example.second:second"), "{error}");
}

#[test]
fn explicit_includes_cannot_bypass_missing_dependency_or_package_cycle_validation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let upstream = temp.path().join("upstream");
    let root_manifest = write_package(
        &root,
        "example.root",
        &["example.up@1.0.0"],
        "  root:\n    overlays: []\n",
    );
    let upstream_manifest = write_package(
        &upstream,
        "example.up",
        &["example.missing@1.0.0"],
        "  up:\n    overlays: []\n",
    );
    let missing = run(&[
        "targets",
        "--manifest",
        root_manifest.to_str().unwrap(),
        "--include",
        upstream_manifest.to_str().unwrap(),
    ]);
    assert!(!missing.status.success());
    assert!(stderr(&missing).contains("package dependency example.missing"));

    fs::write(
        &upstream_manifest,
        fs::read_to_string(&upstream_manifest)
            .unwrap()
            .replace("example.missing@1.0.0", "example.root@1.0.0"),
    )
    .unwrap();
    let cycle = run(&[
        "targets",
        "--manifest",
        root_manifest.to_str().unwrap(),
        "--include",
        upstream_manifest.to_str().unwrap(),
    ]);
    assert!(!cycle.status.success());
    let error = stderr(&cycle);
    assert!(error.contains("package dependency cycle"), "{error}");
    assert!(
        error.contains("example.root -> example.up -> example.root"),
        "{error}"
    );
}

const DECK: &str = r#"deck:
  id: deck.example
  name: Example
  description: Example deck.
  adapter_ids:
    crowdanki:uuid: 43c5ba66-9a65-11e8-90c9-a0481cc15658
note_types:
  note-type.example:
    name: Example
    field_order:
      - field.front
    fields:
      field.front:
        name: Front
    card_template_order:
      - template.example
    card_templates:
      template.example:
        name: Example
        question_format: '{{Front}}'
        answer_format: '{{FrontSide}}'
        adapter_ids: {}
    styling: ''
    adapter_ids:
      crowdanki:uuid: aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa
notes:
  note.example:
    note_type_id: note-type.example
    fields:
      field.front: Example
    tags: []
    adapter_ids:
      crowdanki:guid: example-guid
media: {}
tombstones: []
"#;
