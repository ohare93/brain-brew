use brain_brew_formats::{canonical_yaml, crowdanki};

fn deck_json() -> String {
    let deck = canonical_yaml::from_str(include_str!("../../../fixtures/ug-style/deck.yaml"))
        .expect("fixture parses");
    crowdanki::export_deck(&deck)
        .expect("fixture exports")
        .deck_json
}

#[test]
fn import_plan_is_canonical_and_requires_explicit_approval_before_apply() {
    let source = deck_json();
    let plan = crowdanki::plan_import(source.as_bytes()).expect("source plans");
    assert_eq!(
        plan,
        crowdanki::plan_import(source.as_bytes()).expect("same source replans identically")
    );
    let canonical = plan.to_canonical_json().expect("plan serializes");
    let reread = crowdanki::CrowdAnkiImportPlan::from_bytes(canonical.as_bytes())
        .expect("canonical plan parses");
    assert_eq!(
        canonical,
        reread.to_canonical_json().expect("plan reserializes")
    );

    let unapproved = crowdanki::apply_import_plan(source.as_bytes(), &reread, false)
        .expect_err("automatic IDs still require explicit review approval");
    assert!(unapproved.to_string().contains("--approve-plan"));

    let deck = crowdanki::apply_import_plan(source.as_bytes(), &reread, true)
        .expect("approved matching plan imports");
    assert!(!deck.notes.is_empty());
}

#[test]
fn selected_override_is_applied_but_invalid_or_duplicate_values_fail_closed() {
    let source = deck_json();
    let mut plan = crowdanki::plan_import(source.as_bytes()).expect("source plans");
    let deck_entry = plan
        .entries
        .iter_mut()
        .find(|entry| entry.source_path == "$.name")
        .expect("deck entry exists");
    deck_entry.decision = crowdanki::CrowdAnkiImportPlanDecision::Override {
        stable_id: "deck.reviewed-import".to_owned(),
    };
    let deck = crowdanki::apply_import_plan(source.as_bytes(), &plan, true)
        .expect("selected override applies");
    assert_eq!(deck.id.to_string(), "deck.reviewed-import");

    let mut duplicate = plan.clone();
    duplicate.entries[1].decision = crowdanki::CrowdAnkiImportPlanDecision::Override {
        stable_id: "deck.reviewed-import".to_owned(),
    };
    assert!(
        crowdanki::apply_import_plan(source.as_bytes(), &duplicate, true)
            .expect_err("duplicate override fails")
            .to_string()
            .contains("selected by both")
    );

    let mut invalid = plan;
    invalid.entries[0].decision = crowdanki::CrowdAnkiImportPlanDecision::Override {
        stable_id: "not a stable id".to_owned(),
    };
    assert!(
        crowdanki::apply_import_plan(source.as_bytes(), &invalid, true)
            .expect_err("invalid override fails")
            .to_string()
            .contains("invalid override stable ID")
    );
}

#[test]
fn stale_source_and_unresolved_collisions_are_refused_before_conversion() {
    let source = deck_json();
    let plan = crowdanki::plan_import(source.as_bytes()).expect("source plans");
    assert!(
        crowdanki::apply_import_plan(format!("{source} \n").as_bytes(), &plan, true)
            .expect_err("source byte change is stale")
            .to_string()
            .contains("stale or mutated")
    );

    let mut collision: serde_json::Value = serde_json::from_str(&source).expect("JSON parses");
    collision["media_files"] = serde_json::json!(["foo/bar.png", "foo_bar.png"]);
    let collision_source = serde_json::to_vec(&collision).expect("JSON serializes");
    let collision_plan = crowdanki::plan_import(&collision_source).expect("collision plans");
    assert!(collision_plan.entries.iter().any(|entry| matches!(
        entry.status,
        crowdanki::CrowdAnkiImportPlanStatus::RequiresOverride
    )));
    assert!(
        crowdanki::apply_import_plan(&collision_source, &collision_plan, true)
            .expect_err("unresolved collision is rejected")
            .to_string()
            .contains("unresolved collision")
    );
}
