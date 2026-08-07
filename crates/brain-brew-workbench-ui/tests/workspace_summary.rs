use brain_brew_workbench_ui::WorkspaceSummary;
use serde_json::json;

#[test]
fn summarizes_workspace_metadata_json_for_app_shell() {
    let summary = WorkspaceSummary::from_workspace_json(&json!({
        "manifest": "brainbrew.yaml",
        "languages": {"en": {}, "da": {}},
        "targets": {"en-standard": {}, "da-standard": {}},
        "fingerprints": [{"path": "brainbrew.yaml"}, {"path": "deck.yaml"}, {"path": "da.yaml"}],
        "write_capability": {
            "enabled": false,
            "mode": "read_only",
            "development_build": false,
            "runtime_opt_in": false,
            "warning": "Workbench is read-only."
        },
    }));

    assert_eq!(summary.manifest, "brainbrew.yaml");
    assert_eq!(summary.language_count, 2);
    assert_eq!(summary.target_count, 2);
    assert_eq!(summary.fingerprint_count, 3);
    assert!(!summary.write_capability.enabled);
    assert_eq!(summary.write_capability.mode, "read_only");
    assert!(!summary.write_capability.development_build);
    assert!(!summary.write_capability.runtime_opt_in);
    assert_eq!(summary.write_capability.warning, "Workbench is read-only.");
}
