use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("brainbrew-import-media-cli-{nanos}"));
    fs::create_dir_all(&path).unwrap();
    path
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn strict_import_hands_off_hashed_media_and_force_replaces_the_complete_workspace() {
    let dir = temp_dir();
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ug-style/deck.yaml");
    let crowdanki = dir.join("crowdanki");
    let media_root = dir.join("source-media");
    let plan = dir.join("plan.json");
    let output = dir.join("imported");
    let exported = run(&[
        "export",
        "crowdanki",
        source.to_str().unwrap(),
        "--media-mode",
        "reference-only",
        "--out",
        crowdanki.to_str().unwrap(),
    ]);
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );

    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(crowdanki.join("deck.json")).unwrap()).unwrap();
    for path in value["media_files"].as_array().unwrap() {
        let path = path.as_str().unwrap();
        let file = media_root.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, format!("bytes:{path}")).unwrap();
    }

    let planned = run(&[
        "import",
        "crowdanki",
        "plan",
        crowdanki.to_str().unwrap(),
        "--media-root",
        media_root.to_str().unwrap(),
        "--out",
        plan.to_str().unwrap(),
    ]);
    assert!(
        planned.status.success(),
        "{}",
        String::from_utf8_lossy(&planned.stderr)
    );
    let applied = run(&[
        "import",
        "crowdanki",
        "apply",
        crowdanki.to_str().unwrap(),
        "--plan",
        plan.to_str().unwrap(),
        "--approve-plan",
        "--media-root",
        media_root.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
    ]);
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    let first = value["media_files"][0].as_str().unwrap();
    assert_eq!(
        fs::read(output.join("media").join(first)).unwrap(),
        format!("bytes:{first}").as_bytes()
    );
    assert!(
        fs::read_to_string(output.join("deck.yaml"))
            .unwrap()
            .contains("sha256:")
    );

    fs::write(output.join("media/stale.bin"), b"stale").unwrap();
    let refused = run(&[
        "import",
        "crowdanki",
        "apply",
        crowdanki.to_str().unwrap(),
        "--plan",
        plan.to_str().unwrap(),
        "--approve-plan",
        "--media-root",
        media_root.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
    ]);
    assert!(!refused.status.success());
    let replaced = run(&[
        "import",
        "crowdanki",
        "apply",
        crowdanki.to_str().unwrap(),
        "--plan",
        plan.to_str().unwrap(),
        "--approve-plan",
        "--media-root",
        media_root.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
        "--force",
    ]);
    assert!(
        replaced.status.success(),
        "{}",
        String::from_utf8_lossy(&replaced.stderr)
    );
    assert!(!output.join("media/stale.bin").exists());
}
