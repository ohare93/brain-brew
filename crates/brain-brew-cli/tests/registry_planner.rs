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

fn run_with_cache(args: &[&str], cache: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .env("BRAINBREW_CACHE_DIR", cache)
        .output()
        .expect("brainbrew runs")
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
        &second_manifest,
        fs::read_to_string(&second_manifest)
            .unwrap()
            .replace("version: 1.0.0", "version: 2.0.0"),
    )
    .unwrap();
    let conflicting = run(&[
        "targets",
        "--manifest",
        first_manifest.to_str().unwrap(),
        "--include",
        second_manifest.to_str().unwrap(),
    ]);
    assert!(!conflicting.status.success());
    assert!(stderr(&conflicting).contains("conflicting package versions"));
    fs::write(
        &second_manifest,
        fs::read_to_string(&second_manifest)
            .unwrap()
            .replace("version: 2.0.0", "version: 1.0.0"),
    )
    .unwrap();

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
    assert!(error.contains("example.root@1.0.0"), "{error}");
    assert!(error.contains("example.up@1.0.0"), "{error}");
    assert!(
        error.contains("--depends_on example.up@1.0.0-->"),
        "{error}"
    );
    assert!(
        error.contains("--depends_on example.root@1.0.0-->"),
        "{error}"
    );
    assert!(error.contains(root_manifest.to_str().unwrap()), "{error}");
    assert!(
        error.contains(upstream_manifest.to_str().unwrap()),
        "{error}"
    );
}

#[test]
fn dependency_validation_covers_root_package_root_and_sibling_lock_discovery() {
    let temp = tempfile::tempdir().unwrap();
    let missing_root = temp.path().join("missing-root");
    let missing_manifest = write_package(
        &missing_root,
        "example.missing-root",
        &["example.absent@1.0.0"],
        "  base:\n    overlays: []\n",
    );
    let root_route = run(&["targets", "--manifest", missing_manifest.to_str().unwrap()]);
    assert!(!root_route.status.success());
    assert!(stderr(&root_route).contains("example.absent@1.0.0"));

    let package_root_route = run(&["targets", "--package-root", temp.path().to_str().unwrap()]);
    assert!(!package_root_route.status.success());
    assert!(stderr(&package_root_route).contains("example.absent@1.0.0"));

    let locked = temp.path().join("locked");
    write_package(
        &locked,
        "example.locked",
        &["example.transitive-missing@1.0.0"],
        "  locked:\n    overlays: []\n",
    );
    let consumer = temp.path().join("consumer");
    let consumer_manifest = write_package(
        &consumer,
        "example.consumer",
        &["example.locked@1.0.0"],
        "  consumer:\n    overlays: []\n",
    );
    let lock = consumer.join("brainbrew.lock");
    let cache = temp.path().join("cache");
    let update = run_with_cache(
        &[
            "lock",
            "update",
            "--lock",
            lock.to_str().unwrap(),
            "--package",
            "example.locked",
            "--path",
            locked.to_str().unwrap(),
        ],
        &cache,
    );
    assert!(update.status.success(), "{}", stderr(&update));
    let lock_route = run_with_cache(
        &["targets", "--manifest", consumer_manifest.to_str().unwrap()],
        &cache,
    );
    assert!(!lock_route.status.success());
    assert!(
        stderr(&lock_route).contains("example.transitive-missing@1.0.0"),
        "{}",
        stderr(&lock_route)
    );
}

#[test]
fn package_cycles_include_deterministic_self_edge_traces() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let manifest = write_package(
        &root,
        "example.self",
        &["example.self@1.0.0"],
        "  base:\n    overlays: []\n",
    );
    let output = run(&["targets", "--manifest", manifest.to_str().unwrap()]);
    assert!(!output.status.success());
    let error = stderr(&output);
    let expected = format!(
        "package dependency cycle:\n  example.self@1.0.0 ({path}) --depends_on example.self@1.0.0-->\n  example.self@1.0.0 ({path})",
        path = fs::canonicalize(manifest).unwrap().display()
    );
    assert!(error.contains(&expected), "{error}");
}

#[test]
fn base_compatibility_ranges_use_or_and_semver_prerelease_rules() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().join("base");
    let extension = temp.path().join("extension");
    let base_manifest = write_package(&base, "example.base", &[], "  base:\n    overlays: []\n");
    let extension_manifest = write_package(
        &extension,
        "example.extension",
        &["example.base@1.0.0"],
        "  extended:\n    extends: example.base:base\n    overlays: []\n",
    );
    let source = fs::read_to_string(&extension_manifest).unwrap().replace(
        "  version: 1.0.0\n  depends_on:",
        "  version: 1.0.0\n  base_package: example.base\n  compatible_base_versions:\n    - '>=2, <3'\n    - '>=1, <2'\n  depends_on:",
    );
    fs::write(&extension_manifest, source).unwrap();

    let valid = run(&[
        "targets",
        "--manifest",
        extension_manifest.to_str().unwrap(),
        "--include",
        base_manifest.to_str().unwrap(),
    ]);
    assert!(valid.status.success(), "{}", stderr(&valid));

    fs::write(
        &extension_manifest,
        fs::read_to_string(&extension_manifest)
            .unwrap()
            .replace("    - '>=1, <2'\n", "    - '>=1.0.0-alpha.2, <1.0.0'\n"),
    )
    .unwrap();
    let incompatible = run(&[
        "targets",
        "--manifest",
        extension_manifest.to_str().unwrap(),
        "--include",
        base_manifest.to_str().unwrap(),
    ]);
    assert!(!incompatible.status.success());
    let error = stderr(&incompatible);
    assert!(error.contains("incompatible"), "{error}");
    assert!(error.contains("compatible_base_versions"), "{error}");
}

#[test]
fn registry_rejects_overlay_catalog_id_kind_and_alias_mismatches_before_planning() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let manifest = write_package(&root, "example.catalog", &[], "  base:\n    overlays: []\n");
    fs::write(
        root.join("overlay.yaml"),
        "id: overlay.actual\nkind: extension\n",
    )
    .unwrap();
    fs::write(
        &manifest,
        "package:\n  id: example.catalog\n  version: 1.0.0\nbase: deck.yaml\noverlays:\n  overlay.declared:\n    file: overlay.yaml\n    kind: extension\ntargets:\n  base:\n    overlays: []\n",
    )
    .unwrap();
    let wrong_id = run(&["targets", "--manifest", manifest.to_str().unwrap()]);
    assert!(!wrong_id.status.success());
    assert!(stderr(&wrong_id).contains("catalog identity mismatch"));

    fs::write(
        &manifest,
        fs::read_to_string(&manifest)
            .unwrap()
            .replace("overlay.declared", "overlay.actual")
            .replace("kind: extension", "kind: patch"),
    )
    .unwrap();
    let wrong_kind = run(&["targets", "--manifest", manifest.to_str().unwrap()]);
    assert!(!wrong_kind.status.success());
    assert!(stderr(&wrong_kind).contains("catalog kind mismatch"));

    fs::write(
        &manifest,
        "package:\n  id: example.catalog\n  version: 1.0.0\nbase: deck.yaml\noverlays:\n  overlay.actual:\n    file: overlay.yaml\n    kind: extension\n  overlay.alias:\n    file: overlay.yaml\n    kind: extension\ntargets:\n  base:\n    overlays: []\n",
    )
    .unwrap();
    let alias = run(&["targets", "--manifest", manifest.to_str().unwrap()]);
    assert!(!alias.status.success());
    let error = stderr(&alias);
    assert!(
        error.contains("conflicting identities") || error.contains("catalog identity mismatch"),
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
