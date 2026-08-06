use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const MANIFESTS: [&str; 2] = ["brainbrew-all-csv.yaml", "brainbrew-migrated.yaml"];
const TARGETS: [&str; 3] = ["en-experimental", "de-experimental", "es-experimental"];

#[test]
fn maintained_composable_csv_fixture_certifies_the_incremental_workflow() {
    let temp = TempDir::new().unwrap();
    copy_tree(&fixture_dir(), temp.path());
    let csv_path = temp.path().join("sources/data/countries.csv");
    let original_csv = fs::read(&csv_path).unwrap();

    for source in [
        "brainbrew-all-csv.yaml",
        "brainbrew-migrated.yaml",
        "deck-all-csv.yaml",
        "deck-migrated.yaml",
        "translation-de-all-csv.yaml",
        "translation-de-migrated.yaml",
        "translation-es-all-csv.yaml",
        "translation-es-migrated.yaml",
        "translation-experimental-de.yaml",
        "experimental-all-csv.yaml",
        "experimental-migrated.yaml",
        "note-types.yaml",
        "media.yaml",
    ] {
        success(temp.path(), &["fmt", source]);
    }
    assert_eq!(fs::read(&csv_path).unwrap(), original_csv);
    assert!(
        fs::read_to_string(temp.path().join("deck-all-csv.yaml"))
            .unwrap()
            .contains("notes: !csv")
    );
    assert!(
        fs::read_to_string(temp.path().join("deck-migrated.yaml"))
            .unwrap()
            .contains("- !inline")
    );
    assert!(
        fs::read_to_string(temp.path().join("translation-de-all-csv.yaml"))
            .unwrap()
            .contains("from_csv:")
    );

    for manifest in MANIFESTS {
        success(
            temp.path(),
            &[
                "verify",
                "--manifest",
                manifest,
                "--all-targets",
                "--media-root",
                "media",
            ],
        );
        success(
            temp.path(),
            &[
                "validate",
                "--manifest",
                manifest,
                "--target",
                "de-experimental",
            ],
        );
    }

    fs::create_dir_all(temp.path().join("build")).unwrap();
    for target in TARGETS {
        let all = format!("build/all-{target}.yaml");
        let migrated = format!("build/migrated-{target}.yaml");
        success(
            temp.path(),
            &[
                "compose",
                "--manifest",
                MANIFESTS[0],
                "--target",
                target,
                "--out",
                &all,
            ],
        );
        success(
            temp.path(),
            &[
                "compose",
                "--manifest",
                MANIFESTS[1],
                "--target",
                target,
                "--out",
                &migrated,
            ],
        );
        assert_composed_output(target, &fs::read_to_string(temp.path().join(&all)).unwrap());
        let diff = success(
            temp.path(),
            &["diff", &all, &migrated, "--json", "--exit-code"],
        );
        let diff: Value = serde_json::from_slice(&diff.stdout).unwrap();
        assert_eq!(
            diff["changes"].as_array().unwrap().len(),
            0,
            "target {target}"
        );
    }

    let report = success(
        temp.path(),
        &[
            "translations",
            "--manifest",
            MANIFESTS[0],
            "--target",
            "de-experimental",
            "--overlay",
            "overlay.translation.de",
            "--json",
        ],
    );
    let report: Value = serde_json::from_slice(&report.stdout).unwrap();
    let german = &report["reports"][0];
    assert_eq!(
        german["summary"],
        serde_json::json!({
            "adapter_id_translation": 3,
            "contextual_translation": 3,
            "direct_translation": 6,
            "no_change": 3,
            "target_adaptation": 1,
            "target_deletion": 1,
            "untranslated_fallback": 11
        })
    );
    let csv_owned = german["csv_owned"].as_array().unwrap();
    for path in [
        "notes.note.france.fields.field.continent",
        "notes.note.germany.fields.field.continent",
        "notes.note.spain.fields.field.continent",
    ] {
        assert_csv_unit(csv_owned, "direct", path, "Europe", "Europa");
    }
    assert_csv_unit(
        csv_owned,
        "contextual",
        "notes.note.france.fields.field.shared",
        "Shared",
        "Gemeinsam",
    );
    for path in [
        "notes.note.germany.fields.field.shared",
        "notes.note.spain.fields.field.shared",
    ] {
        assert_csv_unit(csv_owned, "contextual", path, "Shared", "Geteilt");
    }
    assert_csv_unit(
        csv_owned,
        "no_change",
        "notes.note.france.fields.field.capital",
        "Paris",
        "Paris",
    );
    assert_csv_unit(
        csv_owned,
        "deletion",
        "notes.note.france.fields.field.hint",
        "Remove me",
        "",
    );
    assert_csv_unit(
        csv_owned,
        "adaptation",
        "notes.note.germany.fields.field.hint",
        "",
        "Hinzugefügt",
    );
    assert_csv_unit(
        csv_owned,
        "adapter_id",
        "notes.note.france.adapter_ids.crowdanki",
        "guid-fr",
        "guid-fr-de",
    );
    assert!(csv_owned.iter().all(|unit| {
        unit["descriptor"].is_string()
            && unit["file"].is_string()
            && unit["row"].is_number()
            && unit["column"].is_number()
            && unit["path"].is_string()
    }));

    let migrated_report = success(
        temp.path(),
        &[
            "translations",
            "--manifest",
            MANIFESTS[1],
            "--target",
            "de-experimental",
            "--overlay",
            "overlay.translation.de",
            "--json",
        ],
    );
    let migrated_report: Value = serde_json::from_slice(&migrated_report.stdout).unwrap();
    assert_eq!(migrated_report["reports"][0]["summary"], german["summary"]);
    let migrated_csv_owned = migrated_report["reports"][0]["csv_owned"]
        .as_array()
        .unwrap();
    assert_csv_unit(
        migrated_csv_owned,
        "contextual",
        "notes.note.spain.fields.field.shared",
        "Shared",
        "Geteilt",
    );
    assert!(!migrated_csv_owned.iter().any(|unit| {
        unit["path"]
            .as_str()
            .is_some_and(|path| path.starts_with("notes.note.france."))
            || unit["source"] == "Europe"
            || unit["path"] == "notes.note.germany.fields.field.shared"
    }));

    let spanish_report = success(
        temp.path(),
        &[
            "translations",
            "--manifest",
            MANIFESTS[0],
            "--target",
            "es-experimental",
            "--overlay",
            "overlay.translation.es",
            "--json",
        ],
    );
    let spanish_report: Value = serde_json::from_slice(&spanish_report.stdout).unwrap();
    let spanish_csv_owned = spanish_report["reports"][0]["csv_owned"]
        .as_array()
        .unwrap();
    assert_csv_unit(
        spanish_csv_owned,
        "direct",
        "notes.note.france.fields.field.country",
        "France",
        "Francia",
    );
    assert_csv_unit(
        spanish_csv_owned,
        "direct",
        "notes.note.germany.fields.field.shared",
        "Shared",
        "Compartido",
    );
    assert_csv_unit(
        spanish_csv_owned,
        "adaptation",
        "notes.note.germany.fields.field.hint",
        "",
        "Añadido",
    );

    let explain_before = explain(temp.path());
    let sources = explain_before["sources"].as_array().unwrap();
    for name in [
        "countries.yaml",
        "countries-experimental.yaml",
        "regions.yaml",
        "main.csv",
        "countries.csv",
        "hints.csv",
        "guids.csv",
    ] {
        assert!(sources.iter().any(|source| {
            source["path"]
                .as_str()
                .is_some_and(|path| path.ends_with(name))
                && source["sha256"].is_string()
        }));
    }
    let old_table_hash = source_hash(&explain_before, "countries.csv");
    fs::write(
        &csv_path,
        String::from_utf8(original_csv.clone())
            .unwrap()
            .replace("Deutschland", "Bundesrepublik"),
    )
    .unwrap();
    let explain_after = explain(temp.path());
    assert_ne!(old_table_hash, source_hash(&explain_after, "countries.csv"));
    fs::write(&csv_path, &original_csv).unwrap();

    for state in ["all", "migrated"] {
        let manifest = if state == "all" {
            MANIFESTS[0]
        } else {
            MANIFESTS[1]
        };
        let out = format!("build/crowdanki-{state}");
        success(
            temp.path(),
            &[
                "export",
                "crowdanki",
                "--manifest",
                manifest,
                "--target",
                "de-experimental",
                "--out",
                &out,
                "--media-root",
                "media",
            ],
        );
    }
    assert_eq!(
        fs::read(temp.path().join("build/crowdanki-all/deck.json")).unwrap(),
        fs::read(temp.path().join("build/crowdanki-migrated/deck.json")).unwrap()
    );
    for media in [
        "flag-france.svg",
        "flag-germany.svg",
        "flag-spain.svg",
        "map-france.svg",
        "map-germany.svg",
        "map-spain.svg",
    ] {
        assert_eq!(
            fs::read(temp.path().join("build/crowdanki-all/media").join(media)).unwrap(),
            fs::read(
                temp.path()
                    .join("build/crowdanki-migrated/media")
                    .join(media)
            )
            .unwrap()
        );
    }

    let flag = temp.path().join("media/flag-france.svg");
    let original_flag = fs::read(&flag).unwrap();
    fs::write(&flag, b"corrupt").unwrap();
    let failed = run(
        temp.path(),
        &[
            "verify",
            "--manifest",
            MANIFESTS[0],
            "--target",
            "de-experimental",
            "--media-root",
            "media",
        ],
    );
    assert!(!failed.status.success());
    assert!(stderr(&failed).contains("sha256 mismatch"));
    fs::write(&flag, original_flag).unwrap();

    certify_lock_invalidation(temp.path());
}

fn assert_csv_unit(units: &[Value], category: &str, path: &str, source: &str, target: &str) {
    let matches = units
        .iter()
        .filter(|unit| {
            unit["category"] == category
                && unit["path"] == path
                && unit["source"] == source
                && unit["target"] == target
        })
        .count();
    assert_eq!(
        matches, 1,
        "expected exactly one CSV unit ({category}, {path}, {source:?}, {target:?})"
    );
}

fn assert_composed_output(target: &str, output: &str) {
    for anchor in [
        "field.region-code: WE",
        "field.region-code: CE",
        "field.region-code: SW",
        "field.flag: !image media.flag.france",
        "field.map: !image media.map.spain",
    ] {
        assert!(output.contains(anchor), "{target} missing {anchor:?}");
    }
    match target {
        "de-experimental" => {
            for anchor in [
                "field.country: Frankreich",
                "field.country: Deutschland",
                "field.shared: Gemeinsam",
                "field.shared: Geteilt",
                "field.hint: 'Hinzugefügt'",
            ] {
                assert!(output.contains(anchor), "{target} missing {anchor:?}");
            }
        }
        "es-experimental" => {
            for anchor in [
                "field.country: Francia",
                "field.country: Alemania",
                "field.shared: Compartido",
                "field.hint: 'Añadido'",
            ] {
                assert!(output.contains(anchor), "{target} missing {anchor:?}");
            }
        }
        "en-experimental" => assert!(output.contains("field.country: France")),
        _ => unreachable!(),
    }
}

fn explain(root: &Path) -> Value {
    let output = success(
        root,
        &[
            "explain",
            "--manifest",
            MANIFESTS[0],
            "--target",
            "de-experimental",
            "--json",
        ],
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn source_hash(explain: &Value, name: &str) -> String {
    explain["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| {
            source["path"]
                .as_str()
                .is_some_and(|path| path.ends_with(name))
        })
        .unwrap()["sha256"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn certify_lock_invalidation(package: &Path) {
    let consumer = TempDir::new().unwrap();
    fs::write(
        consumer.path().join("brainbrew.yaml"),
        "package:\n  id: fixture.consumer\n  version: 1.0.0\n  base_package: fixture.composable-csv-authoring\n  compatible_base_versions:\n    - '>=1.0.0, <2.0.0'\n  depends_on:\n    - fixture.composable-csv-authoring@1.0.0\nbase: deck.yaml\noverlays: {}\ntargets:\n  de:\n    extends: fixture.composable-csv-authoring:de-experimental\n    overlays: []\n",
    )
    .unwrap();
    fs::write(
        consumer.path().join("deck.yaml"),
        "deck:\n  id: deck.unused\n  name: Unused\n  description: Unused.\n  adapter_ids: {}\nnote_types:\n  note-type.unused:\n    name: Unused\n    field_order:\n      - field.front\n    fields:\n      field.front:\n        name: Front\n    card_template_order: []\n    card_templates: {}\n    styling: ''\n    adapter_ids: {}\nnotes: {}\nmedia: {}\ntombstones: []\n",
    )
    .unwrap();
    success(
        consumer.path(),
        &[
            "lock",
            "update",
            "--package",
            "fixture.composable-csv-authoring",
            "--path",
            package.to_str().unwrap(),
            "--package-manifest",
            "brainbrew-all-csv.yaml",
            "--lock",
            "brainbrew.lock",
        ],
    );
    success(
        consumer.path(),
        &["lock", "verify", "--lock", "brainbrew.lock"],
    );

    let package_output = consumer.path().join("package-de.yaml");
    let consumer_output = consumer.path().join("consumer-de.yaml");
    success(
        package,
        &[
            "compose",
            "--manifest",
            "brainbrew-all-csv.yaml",
            "--target",
            "de-experimental",
            "--out",
            package_output.to_str().unwrap(),
        ],
    );
    success(
        consumer.path(),
        &[
            "compose",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "de",
            "--out",
            consumer_output.to_str().unwrap(),
        ],
    );
    assert_eq!(
        fs::read(&consumer_output).unwrap(),
        fs::read(&package_output).unwrap()
    );
    success(
        consumer.path(),
        &["validate", "--manifest", "brainbrew.yaml", "--target", "de"],
    );
    success(
        consumer.path(),
        &[
            "translations",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "de",
            "--json",
        ],
    );

    let csv = package.join("sources/data/main.csv");
    let original = fs::read(&csv).unwrap();
    let mutated = String::from_utf8(original.clone())
        .unwrap()
        .replace(",CE,", ",CX,");
    assert_ne!(mutated.as_bytes(), original);
    fs::write(&csv, mutated).unwrap();
    let stale = run(
        consumer.path(),
        &["lock", "verify", "--lock", "brainbrew.lock"],
    );
    assert!(!stale.status.success());
    assert!(stderr(&stale).contains("nar_hash mismatch"));
    let live_output = consumer.path().join("live-mutated-de.yaml");
    success(
        package,
        &[
            "compose",
            "--manifest",
            "brainbrew-all-csv.yaml",
            "--target",
            "de-experimental",
            "--out",
            live_output.to_str().unwrap(),
        ],
    );
    assert_ne!(
        fs::read(&live_output).unwrap(),
        fs::read(&package_output).unwrap()
    );
    let stale_consumer_output = consumer.path().join("stale-consumer-de.yaml");
    success(
        consumer.path(),
        &[
            "compose",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "de",
            "--out",
            stale_consumer_output.to_str().unwrap(),
        ],
    );
    assert_eq!(
        fs::read(&stale_consumer_output).unwrap(),
        fs::read(&package_output).unwrap()
    );
    fs::write(csv, original).unwrap();
    success(
        consumer.path(),
        &["lock", "verify", "--lock", "brainbrew.lock"],
    );
}

fn success(root: &Path, args: &[&str]) -> Output {
    let output = run(root, args);
    assert!(output.status.success(), "{args:?}: {}", stderr(&output));
    output
}

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

fn copy_tree(source: &Path, destination: &Path) {
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            fs::create_dir(&target).unwrap();
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/composable-csv-authoring")
}
