use brain_brew_workbench_ui::WorkspaceSummary;
use serde_json::json;

#[test]
fn summarizes_workspace_metadata_json_for_app_shell() {
    let summary = WorkspaceSummary::from_workspace_json(&json!({
        "manifest": "brainbrew.yaml",
        "languages": {"en": {}, "da": {}},
        "targets": {"en-standard": {}, "da-standard": {}},
        "fingerprints": [{"path": "brainbrew.yaml"}, {"path": "deck.yaml"}, {"path": "da.yaml"}],
    }));

    assert_eq!(summary.manifest, "brainbrew.yaml");
    assert_eq!(summary.language_count, 2);
    assert_eq!(summary.target_count, 2);
    assert_eq!(summary.fingerprint_count, 3);
}
