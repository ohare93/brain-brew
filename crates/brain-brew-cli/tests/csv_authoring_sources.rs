use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .current_dir(root)
        .args(args)
        .output()
        .expect("brainbrew runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn run_with_cache(root: &Path, cache: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .current_dir(root)
        .env("BRAINBREW_CACHE_DIR", cache)
        .args(args)
        .output()
        .expect("brainbrew runs")
}

fn write_workspace(root: &Path, descriptor_path: &str, table_path: &str, language: &str) {
    fs::create_dir_all(root.join("authoring/sources/data")).unwrap();
    fs::write(
        root.join("authoring/deck.yaml"),
        format!(
            "deck:\n  id: deck.csv-cli\n  name: CSV CLI\n  description: ''\n  adapter_ids:\n    crowdanki:uuid: 11111111-1111-1111-1111-111111111111\nnote_types:\n  note-type.basic:\n    name: Basic\n    field_order:\n      - field.front\n      - field.back\n    fields:\n      field.front:\n        name: Front\n      field.back:\n        name: Back\n    card_template_order: []\n    card_templates: {{}}\n    styling: ''\n    adapter_ids:\n      crowdanki:uuid: aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa\nnotes: !csv\n  descriptor: {descriptor_path}\n  parameters:\n    language: '{language}'\nmedia: {{}}\ntombstones: []\n"
        ),
    )
    .unwrap();
    fs::write(
        root.join("authoring/sources/descriptor.yaml"),
        format!(
            "version: 1\nprimary_table: main\ntables:\n  main:\n    path: {table_path}\nparameters:\n  language:\n    type: localized_column\n    default: ''\n    separator: ':'\njoins: []\nnote:\n  id: main.stable_id\n  note_type_id: note-type.basic\n  fields:\n    field.front:\n      column: main.front\n      localized_by: language\n      type: scalar\n    field.back:\n      column: main.back\n      type: scalar\n  tags:\n    column: main.tags\n    delimiter: '|'\n  adapter_ids:\n    crowdanki:\n      column: main.guid\n"
        ),
    )
    .unwrap();
    fs::write(
        root.join("authoring/sources/data/notes.csv"),
        "stable_id,front,front:de,back,tags,guid\nnote.one,Hello,Hallo,World,geo,guid-one\n",
    )
    .unwrap();
    fs::write(
        root.join("brainbrew.yaml"),
        "base: authoring/deck.yaml\noverlays: {}\ntargets:\n  standard:\n    overlays: []\n",
    )
    .unwrap();
}

#[test]
fn direct_and_manifest_commands_share_csv_materialization_without_ambient_language() {
    let workspace = TempDir::new().unwrap();
    write_workspace(
        workspace.path(),
        "sources/descriptor.yaml",
        "data/notes.csv",
        "de",
    );

    let direct = run(workspace.path(), &["compose", "authoring/deck.yaml"]);
    assert!(direct.status.success(), "{}", stderr(&direct));
    let manifest = run(
        workspace.path(),
        &[
            "compose",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "standard",
        ],
    );
    assert!(manifest.status.success(), "{}", stderr(&manifest));
    assert_eq!(direct.stdout, manifest.stdout);
    assert!(String::from_utf8_lossy(&direct.stdout).contains("field.front: Hallo"));

    for args in [
        vec!["validate", "authoring/deck.yaml"],
        vec!["diff", "authoring/deck.yaml", "authoring/deck.yaml"],
        vec!["fmt", "authoring/deck.yaml"],
        vec![
            "verify",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "standard",
        ],
        vec![
            "export",
            "crowdanki",
            "authoring/deck.yaml",
            "--out",
            "export",
            "--media-mode",
            "reference-only",
        ],
    ] {
        let output = run(workspace.path(), &args);
        assert!(output.status.success(), "{args:?}: {}", stderr(&output));
    }
}

#[test]
fn fmt_rejects_invalid_authoritative_csv_sources_without_rewriting_the_deck() {
    for case in ["missing", "unsafe", "malformed"] {
        let workspace = TempDir::new().unwrap();
        write_workspace(
            workspace.path(),
            "sources/descriptor.yaml",
            "data/notes.csv",
            "de",
        );
        let deck_path = workspace.path().join("authoring/deck.yaml");
        match case {
            "missing" => {
                fs::remove_file(workspace.path().join("authoring/sources/descriptor.yaml"))
                    .unwrap();
            }
            "unsafe" => {
                let deck = fs::read_to_string(&deck_path)
                    .unwrap()
                    .replace("sources/descriptor.yaml", "../descriptor.yaml");
                fs::write(&deck_path, deck).unwrap();
            }
            "malformed" => {
                fs::write(
                    workspace.path().join("authoring/sources/descriptor.yaml"),
                    "version: [\n",
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        let before = fs::read(&deck_path).unwrap();

        let output = run(workspace.path(), &["fmt", "authoring/deck.yaml"]);
        assert!(!output.status.success(), "{case} unexpectedly formatted");
        assert_eq!(fs::read(&deck_path).unwrap(), before, "{case} rewrote deck");
        let error = stderr(&output);
        assert!(
            !error.contains("unrecognized Brain Brew source file"),
            "{case}: {error}"
        );
        assert!(
            error.contains("notes.descriptor") || error.contains("descriptor.yaml"),
            "{case}: {error}"
        );
    }
}

#[test]
fn csv_descriptor_and_table_paths_fail_closed_before_io() {
    for unsafe_path in [
        "../outside.yaml",
        "./descriptor.yaml",
        "/etc/passwd",
        "C:/Windows/win.ini",
        r"\\server\share\descriptor.yaml",
        r"sources\descriptor.yaml",
        "sources//descriptor.yaml",
    ] {
        let workspace = TempDir::new().unwrap();
        write_workspace(workspace.path(), unsafe_path, "data/notes.csv", "de");
        let output = run(workspace.path(), &["validate", "authoring/deck.yaml"]);
        assert!(
            !output.status.success(),
            "{unsafe_path:?} unexpectedly succeeded"
        );
        let error = stderr(&output);
        assert!(
            error.contains("notes.descriptor path"),
            "{unsafe_path:?}: {error}"
        );
        assert!(
            error.contains(&format!("{unsafe_path:?}")),
            "{unsafe_path:?}: {error}"
        );
    }

    for unsafe_path in [
        "../notes.csv",
        "./notes.csv",
        "/etc/passwd",
        "C:/Windows/win.ini",
        r"\\server\share\notes.csv",
        r"data\notes.csv",
        "data//notes.csv",
    ] {
        let workspace = TempDir::new().unwrap();
        write_workspace(
            workspace.path(),
            "sources/descriptor.yaml",
            unsafe_path,
            "de",
        );
        let output = run(workspace.path(), &["validate", "authoring/deck.yaml"]);
        assert!(
            !output.status.success(),
            "{unsafe_path:?} unexpectedly succeeded"
        );
        let error = stderr(&output);
        assert!(
            error.contains("notes.tables.main path"),
            "{unsafe_path:?}: {error}"
        );
        assert!(
            error.contains(&format!("{unsafe_path:?}")),
            "{unsafe_path:?}: {error}"
        );
    }
}

#[cfg(unix)]
#[test]
fn csv_sources_reject_symlink_escape() {
    use std::os::unix::fs::symlink;

    let workspace = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    write_workspace(
        workspace.path(),
        "sources/descriptor.yaml",
        "escape/notes.csv",
        "de",
    );
    fs::write(outside.path().join("notes.csv"), "secret").unwrap();
    symlink(
        outside.path(),
        workspace.path().join("authoring/sources/escape"),
    )
    .unwrap();

    let output = run(workspace.path(), &["validate", "authoring/deck.yaml"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("escapes the selected root"));
}

#[test]
fn explain_lists_every_csv_input_deterministically_and_hashes_each_file() {
    let workspace = TempDir::new().unwrap();
    write_workspace(
        workspace.path(),
        "sources/descriptor.yaml",
        "data/notes.csv",
        "de",
    );
    let explain = || {
        let output = run(
            workspace.path(),
            &[
                "explain",
                "--manifest",
                "brainbrew.yaml",
                "--target",
                "standard",
                "--json",
            ],
        );
        assert!(output.status.success(), "{}", stderr(&output));
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };

    let first = explain();
    let sources = first["sources"].as_array().expect("sources array");
    assert_eq!(sources.len(), 3);
    assert!(
        sources[0]["path"]
            .as_str()
            .unwrap()
            .ends_with("authoring/deck.yaml")
    );
    assert!(
        sources[1]["path"]
            .as_str()
            .unwrap()
            .ends_with("authoring/sources/descriptor.yaml")
    );
    assert!(
        sources[2]["path"]
            .as_str()
            .unwrap()
            .ends_with("authoring/sources/data/notes.csv")
    );
    assert!(
        sources
            .iter()
            .all(|source| source["sha256"].as_str().unwrap().len() == 64)
    );
    assert_eq!(first, explain());

    let previous_descriptor = sources[1]["sha256"].clone();
    let descriptor_path = workspace.path().join("authoring/sources/descriptor.yaml");
    let descriptor = fs::read_to_string(&descriptor_path).unwrap();
    fs::write(&descriptor_path, format!("{descriptor}\n")).unwrap();
    let descriptor_changed = explain();
    assert_ne!(
        descriptor_changed["sources"][1]["sha256"],
        previous_descriptor
    );
    assert_eq!(
        descriptor_changed["sources"][2]["sha256"],
        first["sources"][2]["sha256"]
    );

    let previous = descriptor_changed["sources"][2]["sha256"].clone();
    fs::write(
        workspace.path().join("authoring/sources/data/notes.csv"),
        "stable_id,front,front:de,back,tags,guid\nnote.one,Hello,Guten Tag,World,geo,guid-one\n",
    )
    .unwrap();
    let changed = explain();
    assert_ne!(changed["sources"][2]["sha256"], previous);
    assert_eq!(
        changed["sources"][0]["sha256"],
        first["sources"][0]["sha256"]
    );
    assert_eq!(
        changed["sources"][1]["sha256"],
        descriptor_changed["sources"][1]["sha256"]
    );
}

#[test]
fn joined_tables_includes_and_package_owned_sources_are_all_planned() {
    let workspace = TempDir::new().unwrap();
    write_workspace(
        workspace.path(),
        "sources/descriptor.yaml",
        "data/notes.csv",
        "de",
    );
    let deck_path = workspace.path().join("authoring/deck.yaml");
    let deck = fs::read_to_string(&deck_path)
        .unwrap()
        .replace("description: ''", "description: !include description.md");
    fs::write(&deck_path, deck).unwrap();
    fs::write(workspace.path().join("description.md"), "CSV package").unwrap();
    fs::write(
        workspace.path().join("authoring/sources/descriptor.yaml"),
        "version: 1\nprimary_table: main\ntables:\n  main:\n    path: data/notes.csv\n  lookup:\n    path: data/lookup.csv\nparameters:\n  language:\n    type: localized_column\n    default: ''\n    separator: ':'\njoins:\n  - left: main.lookup_id\n    right: lookup.lookup_id\nnote:\n  id: main.stable_id\n  note_type_id: note-type.basic\n  fields:\n    field.front:\n      column: lookup.front\n      localized_by: language\n      type: scalar\n    field.back:\n      column: main.back\n      type: scalar\n  tags:\n    column: main.tags\n    delimiter: '|'\n  adapter_ids:\n    crowdanki:\n      column: main.guid\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("authoring/sources/data/notes.csv"),
        "stable_id,lookup_id,back,tags,guid\nnote.one,key.one,World,geo,guid-one\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("authoring/sources/data/lookup.csv"),
        "lookup_id,front,front:de\nkey.one,Hello,Hallo\n",
    )
    .unwrap();
    let manifest = fs::read_to_string(workspace.path().join("brainbrew.yaml")).unwrap();
    fs::write(
        workspace.path().join("brainbrew.yaml"),
        format!("package:\n  id: example.csv\n  version: 1.0.0\n{manifest}"),
    )
    .unwrap();

    let output = run(
        workspace.path(),
        &[
            "explain",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "standard",
            "--json",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let sources = json["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 5);
    assert!(
        sources
            .iter()
            .any(|source| source["kind"] == "scalar_include")
    );
    assert_eq!(
        sources
            .iter()
            .filter(|source| source["kind"] == "csv_table")
            .count(),
        2
    );
    assert!(
        sources
            .iter()
            .all(|source| source["package"]["id"] == "example.csv")
    );
}

#[test]
fn missing_csv_inputs_name_the_declaration_and_referring_source() {
    let workspace = TempDir::new().unwrap();
    write_workspace(
        workspace.path(),
        "sources/missing.yaml",
        "data/notes.csv",
        "de",
    );
    let descriptor = run(workspace.path(), &["validate", "authoring/deck.yaml"]);
    assert!(!descriptor.status.success());
    let error = stderr(&descriptor);
    assert!(error.contains("notes.descriptor path"), "{error}");
    assert!(error.contains("missing.yaml"), "{error}");

    write_workspace(
        workspace.path(),
        "sources/descriptor.yaml",
        "data/missing.csv",
        "de",
    );
    let table = run(workspace.path(), &["validate", "authoring/deck.yaml"]);
    assert!(!table.status.success());
    let error = stderr(&table);
    assert!(error.contains("notes.tables.main path"), "{error}");
    assert!(error.contains("descriptor.yaml"), "{error}");
    assert!(error.contains("missing.csv"), "{error}");
}

#[test]
fn package_lock_verification_invalidates_for_descriptor_and_table_byte_changes() {
    let workspace = TempDir::new().unwrap();
    let package = workspace.path().join("package");
    let consumer = workspace.path().join("consumer");
    fs::create_dir_all(&package).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    write_workspace(&package, "sources/descriptor.yaml", "data/notes.csv", "de");
    let manifest = fs::read_to_string(package.join("brainbrew.yaml")).unwrap();
    fs::write(
        package.join("brainbrew.yaml"),
        format!("package:\n  id: example.csv-lock\n  version: 1.0.0\n{manifest}"),
    )
    .unwrap();
    let lock = consumer.join("brainbrew.lock");
    let cache = workspace.path().join("cache");
    let package_arg = package.to_str().unwrap();
    let lock_arg = lock.to_str().unwrap();
    let update = || {
        run_with_cache(
            workspace.path(),
            &cache,
            &[
                "lock",
                "update",
                "--lock",
                lock_arg,
                "--package",
                "example.csv-lock",
                "--path",
                package_arg,
            ],
        )
    };
    let verify = || {
        run_with_cache(
            workspace.path(),
            &cache,
            &["lock", "verify", "--lock", lock_arg],
        )
    };

    assert!(update().status.success());
    let descriptor = package.join("authoring/sources/descriptor.yaml");
    fs::write(
        &descriptor,
        format!("{}\n", fs::read_to_string(&descriptor).unwrap()),
    )
    .unwrap();
    let descriptor_verify = verify();
    assert!(!descriptor_verify.status.success());
    assert!(stderr(&descriptor_verify).contains("nar_hash mismatch"));

    assert!(update().status.success());
    fs::write(
        package.join("authoring/sources/data/notes.csv"),
        "stable_id,front,front:de,back,tags,guid\nnote.one,Hello,Servus,World,geo,guid-one\n",
    )
    .unwrap();
    let table_verify = verify();
    assert!(!table_verify.status.success());
    assert!(stderr(&table_verify).contains("nar_hash mismatch"));
}

#[test]
fn absent_explicit_localized_header_is_actionable_in_direct_and_manifest_routes() {
    let workspace = TempDir::new().unwrap();
    write_workspace(
        workspace.path(),
        "sources/descriptor.yaml",
        "data/notes.csv",
        "fr",
    );
    for args in [
        vec!["validate", "authoring/deck.yaml"],
        vec![
            "validate",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "standard",
        ],
    ] {
        let output = run(workspace.path(), &args);
        assert!(!output.status.success());
        let error = stderr(&output);
        assert!(error.contains("front:fr"), "{error}");
        assert!(error.contains("notes.csv"), "{error}");
    }
}

#[test]
fn translation_csv_sources_are_materialized_authorized_planned_and_fresh() {
    let workspace = TempDir::new().unwrap();
    fs::create_dir_all(workspace.path().join("overlays/sources/data")).unwrap();
    fs::create_dir_all(workspace.path().join("authoring")).unwrap();
    fs::write(
        workspace.path().join("authoring/deck.yaml"),
        "deck:\n  id: deck.csv-translation-cli\n  name: Fixture\n  description: ''\n  adapter_ids: {}\nnote_types:\n  note-type.basic:\n    name: Basic\n    field_order: [field.front]\n    fields:\n      field.front:\n        name: Front\n    card_template_order: []\n    card_templates: {}\n    styling: ''\n    adapter_ids: {}\nnotes:\n  note.one:\n    note_type_id: note-type.basic\n    fields:\n      field.front: Hello\n    tags: []\n    adapter_ids:\n      crowdanki: guid-one\nmedia: {}\ntombstones: []\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("overlays/de.yaml"),
        "id: overlay.translation.de\nkind: translation\ntranslations:\n  from_csv:\n    - descriptor: sources/descriptor.yaml\n      parameters:\n        language: de\n      exclude:\n        source_texts: []\n        note_ids: []\n        paths: []\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("overlays/extension.yaml"),
        "id: overlay.extension.later\nkind: extension\nnotes:\n  note.two:\n    intent: add\n    note:\n      note_type_id: note-type.basic\n      fields:\n        field.front: Hello\n      tags: []\n      adapter_ids:\n        crowdanki: guid-two\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("overlays/sources/descriptor.yaml"),
        "version: 1\nprimary_table: main\ntables:\n  main:\n    path: data/notes.csv\nparameters:\n  language:\n    type: localized_column\n    default: ''\n    separator: ':'\njoins: []\nnote:\n  id: main.stable_id\n  note_type_id: note-type.basic\n  fields:\n    field.front:\n      column: main.front\n      localized_by: language\n      type: scalar\n  tags:\n    column: main.tags\n    delimiter: '|'\n  adapter_ids:\n    crowdanki:\n      column: main.guid\n      localized_by: language\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("overlays/sources/data/notes.csv"),
        "stable_id,front,front:de,tags,guid,guid:de\nnote.one,Hello,Hallo,,guid-one,guid-one-de\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("brainbrew.yaml"),
        "base: authoring/deck.yaml\noverlays:\n  overlay.translation.de:\n    file: overlays/de.yaml\n    kind: translation\n  overlay.extension.later:\n    file: overlays/extension.yaml\n    kind: extension\ntargets:\n  de:\n    overlays: [overlay.translation.de, overlay.extension.later]\n",
    )
    .unwrap();

    let compose = run(
        workspace.path(),
        &["compose", "--manifest", "brainbrew.yaml", "--target", "de"],
    );
    assert!(compose.status.success(), "{}", stderr(&compose));
    let composed = String::from_utf8_lossy(&compose.stdout);
    assert_eq!(composed.matches("field.front: Hallo").count(), 1);
    assert_eq!(composed.matches("field.front: Hello").count(), 1);

    let direct = run(
        workspace.path(),
        &[
            "compose",
            "authoring/deck.yaml",
            "--overlay",
            "overlays/de.yaml",
            "--overlay",
            "overlays/extension.yaml",
        ],
    );
    assert!(direct.status.success(), "{}", stderr(&direct));
    let direct = String::from_utf8_lossy(&direct.stdout);
    assert_eq!(direct.matches("field.front: Hallo").count(), 1);
    assert_eq!(direct.matches("field.front: Hello").count(), 1);

    let explain = run(
        workspace.path(),
        &[
            "explain",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "de",
            "--json",
        ],
    );
    assert!(explain.status.success(), "{}", stderr(&explain));
    let json: serde_json::Value = serde_json::from_slice(&explain.stdout).unwrap();
    assert_eq!(
        json["sources"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|source| source["kind"] == "csv_descriptor")
            .count(),
        1
    );
    assert_eq!(
        json["sources"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|source| source["kind"] == "csv_table")
            .count(),
        1
    );
    let original_table_hash = json["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["kind"] == "csv_table")
        .unwrap()["sha256"]
        .clone();

    let translations = run(
        workspace.path(),
        &[
            "translations",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "de",
            "--json",
        ],
    );
    assert!(translations.status.success(), "{}", stderr(&translations));
    let translations: serde_json::Value = serde_json::from_slice(&translations.stdout).unwrap();
    let csv_owned = translations["reports"][0]["csv_owned"].as_array().unwrap();
    assert_eq!(csv_owned.len(), 2);
    assert!(
        csv_owned
            .iter()
            .all(|unit| unit["declaration"] == "translations.from_csv[0]")
    );
    assert!(csv_owned.iter().any(|unit| {
        unit["path"] == "notes.note.one.fields.field.front"
            && unit["category"] == "contextual"
            && unit["header"] == "front:de"
            && unit["row"] == 2
            && unit["column"] == 3
    }));
    assert!(csv_owned.iter().any(|unit| {
        unit["path"] == "notes.note.one.adapter_ids.crowdanki"
            && unit["category"] == "adapter_id"
            && unit["header"] == "guid:de"
            && unit["column"] == 6
    }));
    let human = run(
        workspace.path(),
        &[
            "translations",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "de",
        ],
    );
    assert!(human.status.success(), "{}", stderr(&human));
    let human = String::from_utf8_lossy(&human.stdout);
    assert!(human.contains("CSV-owned units: 2"), "{human}");
    assert!(
        human.contains("csv_owned contextual notes.note.one.fields.field.front"),
        "{human}"
    );

    fs::write(
        workspace.path().join("overlays/sources/data/notes.csv"),
        "stable_id,front,front:de,tags,guid,guid:de\nnote.one,Hello,Servus,,guid-one,guid-one-de\n",
    )
    .unwrap();
    let changed = run(
        workspace.path(),
        &["compose", "--manifest", "brainbrew.yaml", "--target", "de"],
    );
    assert!(changed.status.success(), "{}", stderr(&changed));
    let changed_output = String::from_utf8_lossy(&changed.stdout);
    assert_eq!(changed_output.matches("field.front: Servus").count(), 1);
    assert_eq!(changed_output.matches("field.front: Hello").count(), 1);
    let refreshed = run(
        workspace.path(),
        &[
            "explain",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "de",
            "--json",
        ],
    );
    assert!(refreshed.status.success(), "{}", stderr(&refreshed));
    let refreshed: serde_json::Value = serde_json::from_slice(&refreshed.stdout).unwrap();
    let refreshed_table_hash = refreshed["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["kind"] == "csv_table")
        .unwrap()["sha256"]
        .clone();
    assert_ne!(refreshed_table_hash, original_table_hash);
}

#[test]
fn dependency_ordered_csv_translation_and_sparse_overlay_keep_expected_bases_consistent() {
    let workspace = TempDir::new().unwrap();
    fs::create_dir_all(workspace.path().join("overlays/sources")).unwrap();
    fs::write(
        workspace.path().join("deck.yaml"),
        "deck:\n  id: deck.ordered-csv\n  name: Ordered CSV\n  description: ''\n  adapter_ids:\n    crowdanki:uuid: aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa\nnote_types:\n  note-type.basic:\n    name: Basic\n    field_order: [field.front]\n    fields:\n      field.front:\n        name: Front\n    card_template_order: []\n    card_templates: {}\n    styling: ''\n    adapter_ids:\n      crowdanki:uuid: bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb\nnotes:\n  note.one:\n    note_type_id: note-type.basic\n    fields:\n      field.front: Hello\n    tags: []\n    adapter_ids:\n      crowdanki: 11111111-1111-1111-1111-111111111111\nmedia: {}\ntombstones: []\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("overlays/translation.yaml"),
        "id: overlay.translation.de\nkind: translation\ntranslations:\n  from_csv:\n    - descriptor: sources/translation.yaml\n      parameters:\n        language: de\n      exclude:\n        source_texts: []\n        note_ids: []\n        paths: []\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("overlays/experimental.yaml"),
        "id: overlay.extension.experimental\nkind: extension\nfield_additions:\n  note-type.basic:\n    fields:\n      field.experimental: Experimental\n    values:\n      from_csv:\n        - descriptor: sources/experimental.yaml\n          parameters: {}\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("overlays/identity.yaml"),
        "id: overlay.extension.identity\nkind: extension\nnotes:\n  note.one:\n    intent: merge\n    adapter_ids:\n      crowdanki:\n        intent: override\n        value: 33333333-3333-3333-3333-333333333333\n        expected_base:\n          value: 22222222-2222-2222-2222-222222222222\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("overlays/sources/translation.yaml"),
        "version: 1\nprimary_table: main\ntables:\n  main:\n    path: translation.csv\nparameters:\n  language:\n    type: localized_column\n    default: ''\n    separator: ':'\njoins: []\nnote:\n  id: main.stable_id\n  note_type_id: note-type.basic\n  fields:\n    field.front:\n      column: main.front\n      localized_by: language\n      type: scalar\n  tags:\n    column: main.tags\n    delimiter: '|'\n  adapter_ids:\n    crowdanki:\n      column: main.guid\n      localized_by: language\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("overlays/sources/translation.csv"),
        "stable_id,front,front:de,tags,guid,guid:de\nnote.one,Hello,Hallo,,11111111-1111-1111-1111-111111111111,22222222-2222-2222-2222-222222222222\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("overlays/sources/experimental.yaml"),
        "version: 1\nprimary_table: main\ntables:\n  main:\n    path: experimental.csv\nparameters: {}\njoins: []\nnote:\n  id: main.stable_id\n  note_type_id: note-type.basic\n  fields:\n    field.experimental:\n      column: main.experimental\n      type: scalar\n  tags:\n    column: main.tags\n    delimiter: '|'\n  adapter_ids: {}\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("overlays/sources/experimental.csv"),
        "stable_id,experimental,tags\nnote.one,enabled,\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("brainbrew.yaml"),
        "base: deck.yaml\noverlays:\n  overlay.translation.de:\n    file: overlays/translation.yaml\n    kind: translation\n  overlay.extension.experimental:\n    file: overlays/experimental.yaml\n    kind: extension\n    depends_on: [overlay.translation.de]\n  overlay.extension.identity:\n    file: overlays/identity.yaml\n    kind: extension\n    depends_on: [overlay.extension.experimental]\ntargets:\n  de-experimental:\n    overlays: [overlay.extension.identity]\n",
    )
    .unwrap();

    for path in [
        "deck.yaml",
        "overlays/translation.yaml",
        "overlays/experimental.yaml",
        "overlays/identity.yaml",
        "brainbrew.yaml",
    ] {
        let formatted = run(workspace.path(), &["fmt", path]);
        assert!(formatted.status.success(), "{path}: {}", stderr(&formatted));
    }

    let targets = run(
        workspace.path(),
        &["targets", "--manifest", "brainbrew.yaml", "--json"],
    );
    assert!(
        targets.status.success(),
        "targets: {}{}",
        stderr(&targets),
        String::from_utf8_lossy(&targets.stdout)
    );
    let targets: serde_json::Value = serde_json::from_slice(&targets.stdout).unwrap();
    assert_eq!(
        targets["targets"][0]["overlays"]
            .as_array()
            .unwrap()
            .iter()
            .map(|overlay| overlay["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "overlay.translation.de",
            "overlay.extension.experimental",
            "overlay.extension.identity",
        ]
    );

    for args in [
        vec![
            "validate",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "de-experimental",
        ],
        vec![
            "verify",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "de-experimental",
            "--media-mode",
            "reference-only",
        ],
    ] {
        let output = run(workspace.path(), &args);
        assert!(output.status.success(), "{args:?}: {}", stderr(&output));
    }

    let compose = run(
        workspace.path(),
        &[
            "compose",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "de-experimental",
        ],
    );
    assert!(compose.status.success(), "compose: {}", stderr(&compose));
    let composed = String::from_utf8_lossy(&compose.stdout);
    assert!(composed.contains("field.front: Hallo"), "{composed}");
    assert!(
        composed.contains("field.experimental: enabled"),
        "{composed}"
    );
    assert!(
        composed.contains("crowdanki: 33333333-3333-3333-3333-333333333333"),
        "{composed}"
    );

    let export = run(
        workspace.path(),
        &[
            "export",
            "crowdanki",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "de-experimental",
            "--out",
            "export",
            "--media-mode",
            "reference-only",
        ],
    );
    assert!(export.status.success(), "export: {}", stderr(&export));
    let exported = fs::read_to_string(workspace.path().join("export/deck.json")).unwrap();
    assert!(exported.contains("enabled"));
}

#[test]
fn sparse_overlay_csv_values_compose_format_and_register_authoritative_inputs() {
    let workspace = TempDir::new().unwrap();
    fs::create_dir_all(workspace.path().join("overlays/sources")).unwrap();
    fs::write(
        workspace.path().join("deck.yaml"),
        "deck:\n  id: deck.sparse-cli\n  name: Sparse CLI\n  description: ''\n  adapter_ids:\n    crowdanki:uuid: 11111111-1111-1111-1111-111111111111\nnote_types:\n  note-type.country:\n    name: Country\n    field_order: [field.country-name]\n    fields:\n      field.country-name:\n        name: Country\n    card_template_order: []\n    card_templates: {}\n    styling: ''\n    adapter_ids:\n      crowdanki:uuid: 22222222-2222-2222-2222-222222222222\nnotes:\n  note.france:\n    note_type_id: note-type.country\n    fields:\n      field.country-name: France\n    tags: []\n    adapter_ids:\n      crowdanki: guid-france\nmedia: {}\ntombstones: []\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("overlays/experimental.yaml"),
        "id: overlay.extension.experimental\nkind: extension\nfield_additions:\n  note-type.country:\n    fields:\n      field.region-code: Region code\n    values:\n      from_csv:\n        - descriptor: sources/descriptor.yaml\n          parameters: {}\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("overlays/sources/descriptor.yaml"),
        "version: 1\nprimary_table: main\ntables:\n  main:\n    path: regions.csv\nparameters: {}\njoins: []\nnote:\n  id: main.stable_id\n  note_type_id: note-type.country\n  fields:\n    field.region-code:\n      column: main.region_code\n      type: scalar\n  tags:\n    column: main.tags\n    delimiter: '|'\n  adapter_ids: {}\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("overlays/sources/regions.csv"),
        "stable_id,region_code,tags\nnote.france,WE,\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("brainbrew.yaml"),
        "base: deck.yaml\noverlays:\n  overlay.extension.experimental:\n    file: overlays/experimental.yaml\n    kind: extension\ntargets:\n  experimental:\n    overlays: [overlay.extension.experimental]\n",
    )
    .unwrap();

    let composed = run(
        workspace.path(),
        &[
            "compose",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "experimental",
        ],
    );
    assert!(composed.status.success(), "{}", stderr(&composed));
    assert!(String::from_utf8_lossy(&composed.stdout).contains("field.region-code: WE"));

    let direct = run(
        workspace.path(),
        &[
            "compose",
            "deck.yaml",
            "--overlay",
            "overlays/experimental.yaml",
        ],
    );
    assert!(direct.status.success(), "{}", stderr(&direct));
    assert!(String::from_utf8_lossy(&direct.stdout).contains("field.region-code: WE"));
    let validated = run(
        workspace.path(),
        &[
            "validate",
            "deck.yaml",
            "--overlay",
            "overlays/experimental.yaml",
        ],
    );
    assert!(validated.status.success(), "{}", stderr(&validated));
    let exported = run(
        workspace.path(),
        &[
            "export",
            "crowdanki",
            "deck.yaml",
            "--overlay",
            "overlays/experimental.yaml",
            "--out",
            "adhoc-export",
            "--media-mode",
            "reference-only",
        ],
    );
    assert!(exported.status.success(), "{}", stderr(&exported));

    let explain = run(
        workspace.path(),
        &[
            "explain",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "experimental",
            "--json",
        ],
    );
    assert!(explain.status.success(), "{}", stderr(&explain));
    let explain: serde_json::Value = serde_json::from_slice(&explain.stdout).unwrap();
    assert_eq!(
        explain["sources"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|source| source["kind"] == "csv_descriptor")
            .count(),
        1
    );
    assert_eq!(
        explain["sources"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|source| source["kind"] == "csv_table")
            .count(),
        1
    );

    let overlay_path = workspace.path().join("overlays/experimental.yaml");
    let before_csv = fs::read(workspace.path().join("overlays/sources/regions.csv")).unwrap();
    let formatted = run(workspace.path(), &["fmt", "overlays/experimental.yaml"]);
    assert!(formatted.status.success(), "{}", stderr(&formatted));
    assert!(
        fs::read_to_string(&overlay_path)
            .unwrap()
            .contains("values:\n      from_csv:")
    );
    assert_eq!(
        fs::read(workspace.path().join("overlays/sources/regions.csv")).unwrap(),
        before_csv
    );

    fs::write(
        workspace
            .path()
            .join("overlays/sources/translation-descriptor.yaml"),
        "version: 1\nprimary_table: main\ntables:\n  main:\n    path: translations.csv\nparameters:\n  language:\n    type: localized_column\n    default: ''\n    separator: ':'\njoins: []\nnote:\n  id: main.stable_id\n  note_type_id: note-type.country\n  fields:\n    field.country-name:\n      column: main.country\n      localized_by: language\n      type: scalar\n    field.region-code:\n      column: main.region\n      localized_by: language\n      type: scalar\n  tags:\n    column: main.tags\n    delimiter: '|'\n  adapter_ids: {}\n",
    )
    .unwrap();
    fs::write(
        workspace
            .path()
            .join("overlays/sources/translations.csv"),
        "stable_id,country,country:de,region,region:de,tags\nnote.france,France,Frankreich,WE,WE,\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("overlays/de.yaml"),
        "id: overlay.translation.de\nkind: translation\ntranslations:\n  from_csv:\n    - descriptor: sources/translation-descriptor.yaml\n      parameters:\n        language: de\n      exclude:\n        source_texts: []\n        note_ids: []\n        paths: []\n",
    )
    .unwrap();
    let mixed = run(
        workspace.path(),
        &[
            "compose",
            "deck.yaml",
            "--overlay",
            "overlays/experimental.yaml",
            "--overlay",
            "overlays/de.yaml",
        ],
    );
    assert!(mixed.status.success(), "{}", stderr(&mixed));
    let mixed = String::from_utf8_lossy(&mixed.stdout);
    assert!(mixed.contains("field.country-name: Frankreich"), "{mixed}");
    assert!(mixed.contains("field.region-code: WE"), "{mixed}");
}
