use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use brain_brew_core::StableId;
use brain_brew_formats::{canonical_yaml, media};

#[test]
fn ug_style_fixture_composes_exports_imports_and_diffs_semantically() {
    let fixture = fixture_dir();
    let dir = temp_dir("ug-style-fixture");
    let resolved_path = dir.join("resolved.yaml");
    let export_dir = dir.join("crowdanki");
    let imported_path = dir.join("imported.yaml");

    let targets_output = run([
        "targets",
        "--manifest",
        fixture.join("brainbrew.yaml").to_str().unwrap(),
    ]);
    assert!(
        targets_output.status.success(),
        "stderr: {}",
        stderr(&targets_output)
    );
    assert_eq!(stdout(&targets_output), "full-demo\n");

    let verify_output = run([
        "verify",
        "--manifest",
        fixture.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
    ]);
    assert!(
        verify_output.status.success(),
        "stderr: {}",
        stderr(&verify_output)
    );

    let compose_output = run([
        "compose",
        "--manifest",
        fixture.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "full-demo",
        "--out",
        resolved_path.to_str().unwrap(),
    ]);
    assert!(
        compose_output.status.success(),
        "stderr: {}",
        stderr(&compose_output)
    );

    let validate_output = run(["validate", resolved_path.to_str().unwrap()]);
    assert!(
        validate_output.status.success(),
        "stderr: {}",
        stderr(&validate_output)
    );

    let export_output = run([
        "export",
        "crowdanki",
        resolved_path.to_str().unwrap(),
        "--out",
        export_dir.to_str().unwrap(),
    ]);
    assert!(
        export_output.status.success(),
        "stderr: {}",
        stderr(&export_output)
    );
    assert!(stdout(&export_output).contains("omitted tombstones: note.australia"));

    let deck_json = fs::read_to_string(export_dir.join("deck.json")).unwrap();
    let crowdanki: serde_json::Value = serde_json::from_str(&deck_json).unwrap();
    assert_eq!(crowdanki["notes"].as_array().unwrap().len(), 24);
    assert_eq!(crowdanki["media_files"].as_array().unwrap().len(), 50);
    assert_eq!(
        crowdanki["note_models"][0]["flds"]
            .as_array()
            .unwrap()
            .len(),
        10
    );
    assert_eq!(
        crowdanki["note_models"][0]["tmpls"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    assert!(deck_json.contains("Amsterdam (constitutional capital)"));
    assert!(deck_json.contains("Starts with H"));
    assert!(!deck_json.contains("ug-australia-guid"));

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

    let mut expected_export_projection =
        canonical_yaml::from_str(&fs::read_to_string(resolved_path).unwrap()).unwrap();
    media::validate_references(&expected_export_projection)
        .expect("fixture media references validate");
    expected_export_projection
        .notes
        .remove(&sid("note.australia"));
    expected_export_projection.tombstones.clear();
    let imported = canonical_yaml::from_str(&fs::read_to_string(imported_path).unwrap()).unwrap();

    let diff = expected_export_projection.semantic_diff(&imported);
    assert!(diff.is_empty(), "unexpected semantic diff: {diff:#?}");
}

fn run<const N: usize>(args: [&str; N]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .output()
        .expect("command runs")
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

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ug-style")
}

fn sid(value: &str) -> StableId {
    StableId::new(value).expect("test stable id is valid")
}
