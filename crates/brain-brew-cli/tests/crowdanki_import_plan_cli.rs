use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("brainbrew-import-plan-cli-{nanos}"));
    fs::create_dir_all(&path).expect("temp directory");
    path
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .output()
        .expect("command runs")
}

#[test]
fn import_plan_review_apply_and_legacy_migration_are_machine_safe() {
    let dir = temp_dir();
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ug-style/deck.yaml");
    let export = dir.join("crowdanki");
    let plan = dir.join("import-plan.json");
    let output = dir.join("imported.yaml");
    let exported = run(&[
        "export",
        "crowdanki",
        source.to_str().unwrap(),
        "--media-mode",
        "reference-only",
        "--out",
        export.to_str().unwrap(),
    ]);
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );

    let interrupted_plan = dir.join("interrupted-plan.json");
    let interrupted = Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(["import", "crowdanki", "plan"])
        .arg(&export)
        .arg("--out")
        .arg(&interrupted_plan)
        .args(["--media-mode", "reference-only"])
        .env("BRAINBREW_TRANSACTION_FAIL_POINT", "stage:0")
        .env("BRAINBREW_TRANSACTION_FAIL_MODE", "crash")
        .output()
        .expect("interrupted plan runs");
    assert!(!interrupted.status.success());
    let recovered = run(&[
        "import",
        "crowdanki",
        "plan",
        export.to_str().unwrap(),
        "--out",
        interrupted_plan.to_str().unwrap(),
        "--media-mode",
        "reference-only",
    ]);
    assert!(
        recovered.status.success(),
        "{}",
        String::from_utf8_lossy(&recovered.stderr)
    );

    let planned = run(&[
        "import",
        "crowdanki",
        "plan",
        export.to_str().unwrap(),
        "--out",
        plan.to_str().unwrap(),
        "--media-mode",
        "reference-only",
        "--json",
    ]);
    assert!(
        planned.status.success(),
        "{}",
        String::from_utf8_lossy(&planned.stderr)
    );
    let plan_success: serde_json::Value =
        serde_json::from_slice(&planned.stdout).expect("plan JSON");
    assert_eq!(plan_success["schema_version"], 1);
    assert_eq!(plan_success["action"], "plan");

    let review = run(&[
        "import",
        "crowdanki",
        "review",
        "--plan",
        plan.to_str().unwrap(),
    ]);
    assert!(review.status.success());
    assert!(String::from_utf8_lossy(&review.stdout).contains("$.notes[0].guid"));

    let unapproved = run(&[
        "import",
        "crowdanki",
        "apply",
        export.to_str().unwrap(),
        "--plan",
        plan.to_str().unwrap(),
        "--out",
        output.to_str().unwrap(),
        "--media-mode",
        "reference-only",
    ]);
    assert!(!unapproved.status.success());
    assert!(String::from_utf8_lossy(&unapproved.stderr).contains("--approve-plan"));
    assert!(!output.exists());

    let applied = run(&[
        "import",
        "crowdanki",
        "apply",
        export.to_str().unwrap(),
        "--plan",
        plan.to_str().unwrap(),
        "--approve-plan",
        "--out",
        output.to_str().unwrap(),
        "--media-mode",
        "reference-only",
        "--json",
    ]);
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&applied.stdout).unwrap()["action"],
        "apply"
    );
    assert!(output.exists());

    let legacy = run(&[
        "import",
        "crowdanki",
        export.to_str().unwrap(),
        "--accept-suggested-ids",
        "--out",
        dir.join("legacy.yaml").to_str().unwrap(),
    ]);
    assert!(!legacy.status.success());
    assert!(String::from_utf8_lossy(&legacy.stderr).contains("is removed"));
}
