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

fn run_with_cache(args: &[&str], cache: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .env("BRAINBREW_CACHE_DIR", cache)
        .output()
        .expect("brainbrew runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn write_package(
    root: &Path,
    id: &str,
    media_id: &str,
    media_path: &str,
    bytes: &[u8],
    field: &str,
) -> PathBuf {
    fs::create_dir_all(root).unwrap();
    fs::write(root.join(media_path), bytes).unwrap();
    fs::write(
        root.join("deck.yaml"),
        format!(
            r#"deck:
  id: deck.example
  name: Example
  description: Example.
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
        question_format: '{{{{Front}}}}'
        answer_format: '{{{{FrontSide}}}}'
        adapter_ids: {{}}
    styling: ''
    adapter_ids:
      crowdanki:uuid: aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa
notes:
  note.example:
    note_type_id: note-type.example
    fields:
      field.front: '{field}'
    tags: []
    adapter_ids:
      crowdanki:guid: example-guid
media:
  {media_id}:
    path: {media_path}
    sha256: '{}'
tombstones: []
"#,
            sha256_hex(bytes)
        ),
    )
    .unwrap();
    let manifest = root.join("brainbrew.yaml");
    fs::write(
        &manifest,
        format!(
            "package:\n  id: {id}\n  version: 1.0.0\nbase: deck.yaml\noverlays: {{}}\ntargets:\n  base:\n    overlays: []\n"
        ),
    )
    .unwrap();
    for file in [root.join("deck.yaml"), manifest.clone()] {
        let formatted = run(&["fmt", file.to_str().unwrap()]);
        assert!(formatted.status.success(), "{}", stderr(&formatted));
    }
    manifest
}

fn write_consumer(root: &Path, overlay: &str) -> PathBuf {
    fs::create_dir_all(root).unwrap();
    fs::write(root.join("deck.yaml"), EMPTY_ROOT_DECK).unwrap();
    fs::write(root.join("media-overlay.yaml"), overlay).unwrap();
    let manifest = root.join("brainbrew.yaml");
    fs::write(
        &manifest,
        "package:\n  id: example.root\n  version: 1.0.0\n  base_package: example.dep\n  compatible_base_versions:\n    - '>=1.0.0, <2.0.0'\n  depends_on:\n    - example.dep@1.0.0\nbase: deck.yaml\noverlays:\n  overlay.media:\n    file: media-overlay.yaml\n    kind: extension\ntargets:\n  combined:\n    extends: example.dep:base\n    overlays:\n      - overlay.media\n",
    )
    .unwrap();
    for file in [
        root.join("deck.yaml"),
        root.join("media-overlay.yaml"),
        manifest.clone(),
    ] {
        let formatted = run(&["fmt", file.to_str().unwrap()]);
        assert!(formatted.status.success(), "{}", stderr(&formatted));
    }
    manifest
}

#[test]
fn verify_and_export_require_each_owner_root_and_never_substitute_root_bytes() {
    let temp = tempfile::tempdir().unwrap();
    let dep = temp.path().join("dep");
    let root = temp.path().join("root");
    let dep_manifest = write_package(
        &dep,
        "example.dep",
        "media.dep",
        "shared.png",
        b"dependency-bytes",
        "<img src=\"shared.png\" />",
    );
    let root_manifest = write_consumer(
        &root,
        &format!(
            "id: overlay.media\nkind: extension\nmedia:\n  media.root:\n    intent: add\n    path: root.png\n    sha256: '{}'\n",
            sha256_hex(b"root-bytes")
        ),
    );
    let root_media = temp.path().join("root-media");
    let dep_media = temp.path().join("dep-media");
    fs::create_dir_all(&root_media).unwrap();
    fs::create_dir_all(&dep_media).unwrap();
    fs::write(root_media.join("root.png"), b"root-bytes").unwrap();
    // A tempting wrong-root file must never satisfy the dependency declaration.
    fs::write(root_media.join("shared.png"), b"wrong-root-bytes").unwrap();
    fs::write(dep_media.join("shared.png"), b"dependency-bytes").unwrap();

    let missing = run(&[
        "verify",
        "--manifest",
        root_manifest.to_str().unwrap(),
        "--include",
        dep_manifest.to_str().unwrap(),
        "--target",
        "combined",
        "--media-root",
        root_media.to_str().unwrap(),
    ]);
    assert!(!missing.status.success());
    let error = stderr(&missing);
    assert!(error.contains("missing media root"), "{error}");
    assert!(error.contains("example.dep"), "{error}");
    assert!(error.contains("media.dep"), "{error}");

    let qualified_dep = format!("example.dep={}", dep_media.display());
    let verify = run(&[
        "verify",
        "--manifest",
        root_manifest.to_str().unwrap(),
        "--include",
        dep_manifest.to_str().unwrap(),
        "--target",
        "combined",
        "--media-root",
        root_media.to_str().unwrap(),
        "--media-root",
        &qualified_dep,
    ]);
    assert!(verify.status.success(), "{}", stderr(&verify));

    let export_dir = temp.path().join("export");
    let export = run(&[
        "export",
        "crowdanki",
        "--manifest",
        root_manifest.to_str().unwrap(),
        "--include",
        dep_manifest.to_str().unwrap(),
        "--target",
        "combined",
        "--media-root",
        root_media.to_str().unwrap(),
        "--media-root",
        &qualified_dep,
        "--out",
        export_dir.to_str().unwrap(),
    ]);
    assert!(export.status.success(), "{}", stderr(&export));
    assert_eq!(
        fs::read(export_dir.join("media/shared.png")).unwrap(),
        b"dependency-bytes"
    );
    assert_eq!(
        fs::read(export_dir.join("media/root.png")).unwrap(),
        b"root-bytes"
    );
}

#[test]
fn cross_package_path_collisions_and_ambiguous_ownership_transitions_fail() {
    let temp = tempfile::tempdir().unwrap();
    let dep = temp.path().join("dep");
    let root = temp.path().join("root");
    let dep_manifest = write_package(
        &dep,
        "example.dep",
        "media.shared",
        "shared.png",
        b"dep",
        "plain",
    );
    let root_manifest = write_consumer(
        &root,
        &format!(
            "id: overlay.media\nkind: extension\nmedia:\n  media.root:\n    intent: add\n    path: shared.png\n    sha256: '{}'\n",
            sha256_hex(b"root")
        ),
    );
    let collision = run(&[
        "validate",
        "--manifest",
        root_manifest.to_str().unwrap(),
        "--include",
        dep_manifest.to_str().unwrap(),
        "--target",
        "combined",
    ]);
    assert!(!collision.status.success());
    assert!(stderr(&collision).contains("media path/output collision"));

    fs::write(
        root.join("media-overlay.yaml"),
        format!(
            "id: overlay.media\nkind: extension\nmedia:\n  media.shared:\n    intent: merge\n    path: root.png\n    sha256: '{}'\n",
            sha256_hex(b"root")
        ),
    )
    .unwrap();
    let formatted = run(&["fmt", root.join("media-overlay.yaml").to_str().unwrap()]);
    assert!(formatted.status.success(), "{}", stderr(&formatted));
    let ambiguous = run(&[
        "validate",
        "--manifest",
        root_manifest.to_str().unwrap(),
        "--include",
        dep_manifest.to_str().unwrap(),
        "--target",
        "combined",
    ]);
    assert!(!ambiguous.status.success());
    assert!(
        stderr(&ambiguous).contains("cross-package media stable-ID collision"),
        "{}",
        stderr(&ambiguous)
    );

    fs::write(
        root.join("media-overlay.yaml"),
        format!(
            "id: overlay.media\nkind: extension\nmedia:\n  media.shared:\n    intent: replace\n    path: root.png\n    sha256: '{}'\n    expected_base: entity_present\n",
            sha256_hex(b"root")
        ),
    )
    .unwrap();
    let formatted = run(&["fmt", root.join("media-overlay.yaml").to_str().unwrap()]);
    assert!(formatted.status.success(), "{}", stderr(&formatted));
    let explicit_transition = run(&[
        "validate",
        "--manifest",
        root_manifest.to_str().unwrap(),
        "--include",
        dep_manifest.to_str().unwrap(),
        "--target",
        "combined",
    ]);
    assert!(
        explicit_transition.status.success(),
        "explicit replace transfers final ownership: {}",
        stderr(&explicit_transition)
    );
}

#[test]
fn media_mutation_rejects_locked_declarations_without_changing_cache_or_source() {
    let temp = tempfile::tempdir().unwrap();
    let dep = temp.path().join("dep");
    let root = temp.path().join("root");
    write_package(
        &dep,
        "example.dep",
        "media.dep",
        "shared.png",
        b"dependency-bytes",
        "<img src=\"shared.png\" />",
    );
    let root_manifest = write_consumer(&root, "id: overlay.media\nkind: extension\n");
    let cache = temp.path().join("cache");
    let lock = root.join("brainbrew.lock");
    let update = run_with_cache(
        &[
            "lock",
            "update",
            "--lock",
            lock.to_str().unwrap(),
            "--package",
            "example.dep",
            "--path",
            dep.to_str().unwrap(),
        ],
        &cache,
    );
    assert!(update.status.success(), "{}", stderr(&update));
    let source_before = fs::read(dep.join("deck.yaml")).unwrap();
    let cache_before = tree_bytes(&cache);
    let dep_root_arg = format!("example.dep={}", dep.display());

    let hash = run_with_cache(
        &[
            "media",
            "hash",
            "--manifest",
            root_manifest.to_str().unwrap(),
            "--target",
            "combined",
            "--media-root",
            root.to_str().unwrap(),
            "--media-root",
            &dep_root_arg,
        ],
        &cache,
    );
    assert!(!hash.status.success());
    let error = stderr(&hash);
    assert!(error.contains("read-only"), "{error}");
    assert!(error.contains("locked"), "{error}");
    assert_eq!(fs::read(dep.join("deck.yaml")).unwrap(), source_before);
    assert_eq!(tree_bytes(&cache), cache_before);
}

fn tree_bytes(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, current: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        if !current.exists() {
            return;
        }
        for entry in fs::read_dir(current).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out);
    out
}

use std::collections::BTreeMap;

const EMPTY_ROOT_DECK: &str = r#"deck:
  id: deck.unused
  name: Unused
  description: Unused.
  adapter_ids: {}
note_types:
  note-type.unused:
    name: Unused
    field_order:
      - field.front
    fields:
      field.front:
        name: Front
    card_template_order:
      - template.unused
    card_templates:
      template.unused:
        name: Unused
        question_format: '{{Front}}'
        answer_format: '{{FrontSide}}'
        adapter_ids: {}
    styling: ''
    adapter_ids: {}
notes:
  note.unused:
    note_type_id: note-type.unused
    fields:
      field.front: Unused
    tags: []
    adapter_ids: {}
media: {}
tombstones: []
"#;
