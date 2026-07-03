use std::collections::BTreeSet;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flate2::Compression;
use flate2::write::GzEncoder;
use tar::Builder;

#[test]
fn top_level_help_includes_examples() {
    let output = run(["--help"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Usage:"));
    assert!(out.contains("Examples:"));
    assert!(out.contains("brainbrew targets --manifest brainbrew.yaml"));
    assert!(
        out.contains("brainbrew export crowdanki --manifest brainbrew.yaml --target de-extended")
    );
}

#[test]
fn command_help_includes_focused_examples() {
    let output = run(["compose", "--help"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Usage:"));
    assert!(out.contains(
        "brainbrew compose --manifest brainbrew.yaml --target da-standard --out build/da.yaml"
    ));
}

#[test]
fn workbench_help_includes_serve_entrypoint() {
    let top = run(["--help"]);
    assert!(top.status.success(), "stderr: {}", stderr(&top));
    assert!(stdout(&top).contains("workbench"));

    let command = run(["workbench", "--help"]);
    assert!(command.status.success(), "stderr: {}", stderr(&command));
    let out = stdout(&command);
    assert!(out.contains("brainbrew workbench serve --manifest brainbrew.yaml"));
    assert!(out.contains("--port"));
    assert!(out.contains("--no-open"));
    assert!(out.contains("--dev-assets"));
    assert!(out.contains("target/workbench-ui"));

    let serve = run(["workbench", "serve", "--help"]);
    assert!(serve.status.success(), "stderr: {}", stderr(&serve));
    assert!(stdout(&serve).contains("brainbrew workbench serve --manifest brainbrew.yaml"));
}

#[test]
fn workbench_serve_exposes_workspace_metadata_api() {
    let dir = temp_dir("workbench-api");
    write_workbench_workspace(&dir);
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let health = get_json(&server.url("/api/health"));
    assert_eq!(health["status"], "ok");
    assert_eq!(
        health["manifest"],
        dir.join("brainbrew.yaml").display().to_string()
    );

    let workspace = get_json(&server.url("/api/workspace"));
    assert_eq!(
        workspace["manifest"],
        dir.join("brainbrew.yaml").display().to_string()
    );
    assert_eq!(workspace["languages"]["en"]["display_name"], "English");
    assert_eq!(workspace["languages"]["en"]["source"], true);
    assert_eq!(
        workspace["languages"]["da"]["translation_overlays"]["base"],
        "overlay.translation.da"
    );
    assert_eq!(
        workspace["languages"]["da"]["targets"]["standard"],
        "da-standard"
    );
    assert_eq!(
        workspace["target_labels"]["da-standard"][0]["language"],
        "da"
    );
    assert_eq!(
        workspace["target_labels"]["da-standard"][0]["label"],
        "standard"
    );
    assert_eq!(
        workspace["translation_profile"]["structural_fields"][0],
        "field.flag"
    );
    let fingerprints = workspace["fingerprints"].as_array().unwrap();
    assert!(
        fingerprints
            .iter()
            .any(|entry| entry["path"] == "brainbrew.yaml")
    );
    assert!(
        fingerprints
            .iter()
            .any(|entry| entry["path"] == "deck.yaml")
    );
    assert!(fingerprints.iter().any(|entry| entry["path"] == "da.yaml"));
    assert!(
        fingerprints
            .iter()
            .all(|entry| entry["sha256"].as_str().unwrap().len() == 64)
    );
    let original_da_fingerprint = fingerprints
        .iter()
        .find(|entry| entry["path"] == "da.yaml")
        .unwrap()["sha256"]
        .as_str()
        .unwrap()
        .to_owned();

    fs::write(
        dir.join("da.yaml"),
        r#"id: overlay.translation.da
kind: translation
translations:
  direct:
    Finland: Suomi
"#,
    )
    .unwrap();
    let updated = get_json(&server.url("/api/workspace"));
    let updated_da_fingerprint = updated["fingerprints"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["path"] == "da.yaml")
        .unwrap()["sha256"]
        .as_str()
        .unwrap();
    assert_ne!(updated_da_fingerprint, original_da_fingerprint);
}

#[test]
fn workbench_new_language_scaffold_preview_write_and_initial_edit() {
    let dir = temp_dir("workbench-new-language");
    write_workbench_workspace(&dir);
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let preview = get_json(&server.url(
        "/api/workbench/new-language-preview?template=da&code=nb&display_name=Norwegian%20Bokmal",
    ));
    assert_eq!(preview["validation"]["ok"], true);
    assert_eq!(preview["language"]["code"], "nb");
    assert_eq!(preview["groups"][0]["label"], "base");
    assert_eq!(preview["groups"][0]["selected"], true);
    assert_eq!(preview["groups"][0]["overlay_id"], "overlay.translation.nb");
    assert_eq!(preview["groups"][0]["file"], "overlays/languages/nb.yaml");
    assert_eq!(preview["targets"][0]["target_id"], "nb-standard");
    assert!(
        !dir.join("overlays/languages/nb.yaml").exists(),
        "preview must not write overlay files"
    );
    assert!(
        !fs::read_to_string(dir.join("brainbrew.yaml"))
            .unwrap()
            .contains("nb-standard")
    );

    let created = post_json(
        &server.url("/api/workbench/new-language"),
        preview["draft"].clone(),
    );
    assert_eq!(created["created"], true);
    let manifest = fs::read_to_string(dir.join("brainbrew.yaml")).unwrap();
    assert!(manifest.contains("overlay.translation.nb:"));
    assert!(manifest.contains("file: overlays/languages/nb.yaml"));
    assert!(manifest.contains("nb-standard:"));
    assert!(manifest.contains("nb:\n    display_name: Norwegian Bokmal"));
    let overlay = fs::read_to_string(dir.join("overlays/languages/nb.yaml")).unwrap();
    assert_eq!(
        overlay,
        "id: overlay.translation.nb\nkind: translation\ntranslations: {}\n"
    );

    let workspace = get_json(&server.url("/api/workspace"));
    assert_eq!(
        workspace["languages"]["nb"]["display_name"],
        "Norwegian Bokmal"
    );
    assert_eq!(
        workspace["languages"]["nb"]["translation_overlays"]["base"],
        "overlay.translation.nb"
    );
    assert!(
        workspace["fingerprints"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["path"] == "overlays/languages/nb.yaml")
    );

    let pivot = get_json(&server.url("/api/workbench/note-pivot?language=nb&target=standard"));
    assert_eq!(pivot["progress"]["total"], 2);
    assert_eq!(pivot["progress"]["complete"], 0);
    assert_eq!(pivot["progress"]["missing"], 2);

    let applied = post_json(
        &server.url("/api/workbench/apply"),
        serde_json::json!({
            "language": "nb",
            "target": "standard",
            "overlay": "base",
            "edits": [{
                "path": "notes.note.finland.fields.field.capital",
                "source": "Helsinki",
                "value": "Helsingfors",
                "mode": "direct"
            }]
        }),
    );
    assert_eq!(applied["validation"]["ok"], true);
    let overlay = fs::read_to_string(dir.join("overlays/languages/nb.yaml")).unwrap();
    assert!(overlay.contains("Helsinki: Helsingfors"));
}

#[test]
fn workbench_new_language_scaffold_can_deselect_template_overlay_groups() {
    let dir = temp_dir("workbench-new-language-groups");
    fs::write(dir.join("deck.yaml"), SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        dir.join("da.yaml"),
        r#"id: overlay.translation.da
kind: translation
translations:
  direct:
    Finland: Finland
"#,
    )
    .unwrap();
    fs::write(
        dir.join("hardcore-da.yaml"),
        r#"id: overlay.translation.hardcore.da
kind: translation
translations:
  direct:
    Helsinki: Helsingfors
"#,
    )
    .unwrap();
    fs::write(
        dir.join("brainbrew.yaml"),
        r#"base: deck.yaml
overlays:
  overlay.translation.da:
    file: da.yaml
    kind: translation
  overlay.translation.hardcore.da:
    file: hardcore-da.yaml
    kind: translation
targets:
  da-standard:
    overlays:
      - overlay.translation.da
      - overlay.translation.hardcore.da
  en-standard:
    overlays: []
languages:
  da:
    display_name: Danish
    translation_overlays:
      base: overlay.translation.da
      hardcore: overlay.translation.hardcore.da
    primary_target: standard
    targets:
      standard: da-standard
  en:
    display_name: English
    source: true
    primary_target: standard
    targets:
      standard: en-standard
translation_profile:
  structural_fields:
    - field.flag
"#,
    )
    .unwrap();
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let preview = get_json(
        &server
            .url("/api/workbench/new-language-preview?template=da&code=nb&display_name=Norwegian"),
    );
    let groups = preview["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 2);
    assert!(groups.iter().all(|group| group["selected"] == true));
    assert_eq!(groups[1]["label"], "hardcore");
    assert_eq!(groups[1]["overlay_id"], "overlay.translation.hardcore.nb");
    assert_eq!(groups[1]["file"], "overlays/languages/hardcore/nb.yaml");

    let mut draft = preview["draft"].clone();
    draft["groups"][1]["selected"] = serde_json::json!(false);
    let created = post_json(&server.url("/api/workbench/new-language"), draft);
    assert_eq!(created["created"], true);
    let manifest = fs::read_to_string(dir.join("brainbrew.yaml")).unwrap();
    assert!(manifest.contains("overlay.translation.nb:"));
    assert!(!manifest.contains("overlay.translation.hardcore.nb"));
    assert!(dir.join("overlays/languages/nb.yaml").exists());
    assert!(!dir.join("overlays/languages/hardcore/nb.yaml").exists());
    let workspace = get_json(&server.url("/api/workspace"));
    assert_eq!(
        workspace["languages"]["nb"]["translation_overlays"]["base"],
        "overlay.translation.nb"
    );
    assert!(workspace["languages"]["nb"]["translation_overlays"]["hardcore"].is_null());
}

#[test]
fn workbench_serve_uses_available_port_and_embedded_assets_by_default() {
    let dir = temp_dir("workbench-default-assets");
    write_workbench_workspace(&dir);

    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--no-open",
    ]);

    let response = ureq::get(&server.url("/")).call().expect("GET / succeeds");
    assert_eq!(response.status(), 200);
    assert!(
        response
            .header("content-type")
            .unwrap_or_default()
            .starts_with("text/html")
    );
    let index = response.into_string().unwrap();
    assert!(index.contains("Brain Brew Deck Workbench"));
    assert!(index.contains("TrunkApplicationStarted"));

    let js_path = quoted_path_containing(&index, ".js");
    let js_response = ureq::get(&server.url(&js_path))
        .call()
        .expect("GET embedded JS succeeds");
    assert_eq!(js_response.status(), 200);
    assert!(
        js_response
            .header("content-type")
            .unwrap_or_default()
            .contains("javascript")
    );
    assert!(js_response.into_string().unwrap().contains("wasm_bindgen"));

    let wasm_path = quoted_path_containing(&index, ".wasm");
    let wasm_response = ureq::get(&server.url(&wasm_path))
        .call()
        .expect("GET embedded WASM succeeds");
    assert_eq!(wasm_response.status(), 200);
    assert_eq!(
        wasm_response.header("content-type").unwrap_or_default(),
        "application/wasm"
    );
    let mut wasm_bytes = Vec::new();
    wasm_response
        .into_reader()
        .read_to_end(&mut wasm_bytes)
        .unwrap();
    assert!(wasm_bytes.starts_with(b"\0asm"));
}

#[test]
fn workbench_navigation_lists_are_paginated_and_compact() {
    let dir = temp_dir("workbench-navigation-lists");
    write_workbench_repeated_source_workspace(&dir);
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let notes = get_json(
        &server.url("/api/workbench/note-list?language=da&target=standard&limit=1&offset=0"),
    );
    assert_eq!(notes["language"]["code"], "da");
    assert_eq!(notes["target"]["label"], "standard");
    assert_eq!(notes["total"], 2);
    assert_eq!(notes["limit"], 1);
    assert_eq!(notes["offset"], 0);
    assert_eq!(notes["has_more"], true);
    assert_eq!(notes["progress"]["total"], 4);
    assert_eq!(notes["rows"].as_array().unwrap().len(), 1);
    assert!(
        notes["rows"][0]["note_id"]
            .as_str()
            .unwrap()
            .starts_with("note.")
    );
    assert!(notes["rows"][0].get("fields").is_none());
    assert!(notes["rows"][0].get("source_preview").is_none());

    let detail = get_json(&server.url(&format!(
        "/api/workbench/note-detail?language=da&target=standard&note={}",
        notes["rows"][0]["note_id"].as_str().unwrap()
    )));
    assert_eq!(detail["notes"].as_array().unwrap().len(), 1);
    assert_eq!(detail["notes"][0]["note_id"], notes["rows"][0]["note_id"]);
    assert!(detail["notes"][0]["fields"].as_array().unwrap().len() >= 2);

    let notes_page_2 = get_json(
        &server.url("/api/workbench/note-list?language=da&target=standard&limit=1&offset=1"),
    );
    assert_eq!(notes_page_2["total"], 2);
    assert_eq!(notes_page_2["rows"].as_array().unwrap().len(), 1);
    assert_eq!(notes_page_2["has_more"], false);
    assert_ne!(
        notes["rows"][0]["note_id"],
        notes_page_2["rows"][0]["note_id"]
    );

    let cards = get_json(
        &server.url("/api/workbench/card-list?language=da&target=standard&limit=1&offset=0"),
    );
    assert_eq!(cards["total"], 2);
    assert_eq!(cards["progress"]["total"], 2);
    assert_eq!(cards["rows"].as_array().unwrap().len(), 1);
    assert!(cards["rows"][0]["card_id"].as_str().unwrap().contains("::"));
    assert!(cards["rows"][0].get("fields").is_none());
    assert!(cards["rows"][0].get("source_preview").is_none());

    let strings = get_json(
        &server
            .url("/api/workbench/source-string-list?language=da&target=standard&limit=1&offset=0"),
    );
    assert_eq!(strings["total"], 3);
    assert_eq!(strings["progress"]["total"], 3);
    assert_eq!(strings["rows"].as_array().unwrap().len(), 1);
    assert!(!strings["rows"][0]["source"].as_str().unwrap().is_empty());
    assert!(strings["rows"][0].get("occurrences").is_none());

    let full_pivot = get_json(&server.url("/api/workbench/card-pivot?language=da&target=standard"));
    assert_eq!(full_pivot["cards"].as_array().unwrap().len(), 2);
    assert!(full_pivot["selected_card"].get("fields").is_some());
}

#[test]
fn workbench_navigation_titles_follow_note_type_field_order() {
    let manifest = workspace_root().join("fixtures/ultimate-geography/brainbrew.yaml");
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        manifest.to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let notes = get_json(
        &server.url("/api/workbench/note-list?language=de&target=standard&limit=8&offset=0"),
    );
    let titles = notes["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["title"].as_str().unwrap_or_default().to_owned())
        .collect::<Vec<_>>();
    assert!(titles.iter().all(|title| !title.trim().is_empty()));
    assert_eq!(titles.first().map(String::as_str), Some("Abkhazia"));
    assert!(!titles.iter().any(|title| title == "Sukhumi"));
}

#[test]
fn workbench_optional_metadata_list_is_paginated_and_keeps_full_totals() {
    let dir = temp_dir("workbench-optional-metadata-list");
    write_workbench_optional_metadata_workspace(&dir);
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let optional =
        get_json(&server.url("/api/workbench/metadata-list?language=da&limit=1&offset=0"));
    assert_eq!(optional["language"]["code"], "da");
    assert_eq!(optional["limit"], 1);
    assert_eq!(optional["offset"], 0);
    assert!(optional["total"].as_u64().unwrap() > 1);
    assert_eq!(optional["has_more"], true);
    assert_eq!(optional["rows"].as_array().unwrap().len(), 1);
    assert_eq!(optional["main_progress"]["total"], 2);
    assert_eq!(optional["metadata_progress"]["total"], optional["total"]);
    assert!(optional["rows"][0]["path"].as_str().unwrap().contains('.'));
    assert!(optional["rows"][0].get("target").is_none());
}

#[test]
fn workbench_navigation_lists_reject_invalid_pagination() {
    let dir = temp_dir("workbench-navigation-list-errors");
    write_workbench_repeated_source_workspace(&dir);
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    assert_get_status_contains(
        &server.url("/api/workbench/note-list?limit=0"),
        400,
        "invalid pagination limit",
    );
    assert_get_status_contains(
        &server.url("/api/workbench/source-string-list?offset=-1"),
        400,
        "invalid pagination offset",
    );
    assert_get_status_contains(
        &server.url("/api/workbench/card-list?limit=9999"),
        400,
        "invalid pagination limit",
    );
}

#[test]
fn workbench_note_pivot_exposes_target_translation_data_and_media() {
    let dir = temp_dir("workbench-note-pivot");
    write_workbench_workspace(&dir);
    fs::create_dir_all(dir.join("media/flags")).unwrap();
    fs::write(dir.join("media/flags/fi.png"), b"png").unwrap();
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let pivot = get_json(&server.url("/api/workbench/note-pivot?language=da&target=standard"));
    assert_eq!(pivot["language"]["code"], "da");
    assert_eq!(pivot["target"]["id"], "da-standard");
    assert_eq!(pivot["overlay"]["id"], "overlay.translation.da");
    assert_eq!(pivot["selection_options"]["languages"][0]["code"], "da");
    assert_eq!(
        pivot["selection_options"]["targets"][0]["label"],
        "standard"
    );
    assert_eq!(pivot["selection_options"]["overlays"][0]["label"], "base");
    assert_eq!(pivot["progress"]["total"], 2);
    assert_eq!(pivot["progress"]["complete"], 1);
    assert_eq!(pivot["progress"]["missing"], 1);
    assert_eq!(pivot["overlay_badges"][0]["label"], "base");
    let note = &pivot["notes"][0];
    assert_eq!(note["note_id"], "note.finland");
    assert!(
        note["source_preview"]["cards"][0]["question_html"]
            .as_str()
            .unwrap()
            .contains("Finland")
    );
    assert!(
        note["source_preview"]["cards"][0]["answer_html"]
            .as_str()
            .unwrap()
            .contains("Helsinki")
    );
    let capital = note["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["field_id"] == "field.capital")
        .unwrap();
    assert_eq!(capital["status"], "untranslated_fallback");
    assert_eq!(capital["occurrence_count"], 1);
    assert_eq!(capital["controls"][0], "direct");
    let flag = note["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["field_id"] == "field.flag")
        .unwrap();
    assert_eq!(flag["structural"], true);
    assert_eq!(flag["editable"], false);
    assert_eq!(flag["source_editable"], true);

    let missing = get_json(&server.url("/api/workbench/note-pivot?language=da&filter=missing"));
    assert_eq!(missing["notes"].as_array().unwrap().len(), 1);

    let media = ureq::get(&server.url("/api/media/flags/fi.png"))
        .call()
        .expect("GET declared media succeeds");
    assert_eq!(media.status(), 200);
    assert_eq!(
        media.header("content-type").unwrap_or_default(),
        "image/png"
    );
}

#[test]
fn workbench_media_cache_refreshes_after_workspace_edit_and_rejects_undeclared_paths() {
    let dir = temp_dir("workbench-media-cache-refresh");
    write_workbench_workspace(&dir);
    fs::create_dir_all(dir.join("media/flags")).unwrap();
    fs::write(dir.join("media/flags/fi.png"), b"fi").unwrap();
    fs::write(dir.join("media/flags/se.png"), b"se").unwrap();
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let original = ureq::get(&server.url("/api/media/flags/fi.png"))
        .call()
        .expect("initial declared media request warms the cache");
    assert_eq!(original.status(), 200);

    std::thread::sleep(Duration::from_millis(20));
    let deck = fs::read_to_string(dir.join("deck.yaml")).unwrap();
    fs::write(
        dir.join("deck.yaml"),
        deck.replace(
            "  media.flags-fi-png:\n    path: flags/fi.png\n    sha256: ''",
            "  media.flags-fi-png:\n    path: flags/fi.png\n    sha256: ''\n  media.flags-se-png:\n    path: flags/se.png\n    sha256: ''",
        ),
    )
    .unwrap();

    let refreshed = ureq::get(&server.url("/api/media/flags/se.png"))
        .call()
        .expect("newly declared media succeeds after cache invalidation");
    assert_eq!(refreshed.status(), 200);
    assert_get_status_contains(
        &server.url("/api/media/flags/nope.png"),
        400,
        "unknown media asset",
    );
}

#[test]
fn workbench_media_can_load_from_media_root_or_placeholder() {
    let dir = temp_dir("workbench-media-root");
    write_workbench_workspace(&dir);
    let external_media = dir.join("external-media");
    fs::create_dir_all(external_media.join("flags")).unwrap();
    fs::write(external_media.join("flags/fi.png"), b"png").unwrap();
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--media-root",
        external_media.to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let media = ureq::get(&server.url("/api/media/flags/fi.png"))
        .call()
        .expect("GET declared media from media root succeeds");
    assert_eq!(media.status(), 200);
    assert_eq!(
        media.header("content-type").unwrap_or_default(),
        "image/png"
    );

    let missing_dir = temp_dir("workbench-missing-media-placeholder");
    write_workbench_workspace(&missing_dir);
    let missing_server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        missing_dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);
    let placeholder = ureq::get(&missing_server.url("/api/media/flags/fi.png"))
        .call()
        .expect("GET declared missing media returns visible placeholder");
    assert_eq!(placeholder.status(), 200);
    assert_eq!(
        placeholder.header("content-type").unwrap_or_default(),
        "image/svg+xml"
    );
    let body = placeholder.into_string().unwrap();
    assert!(body.contains("Missing media asset"));
    assert!(body.contains("flags/fi.png"));
}

#[test]
fn workbench_media_auto_discovers_external_deck_media() {
    let root = temp_dir("workbench-auto-external-media");
    let manifest_dir = root.join("projects/deck-fixture");
    fs::create_dir_all(&manifest_dir).unwrap();
    write_workbench_workspace(&manifest_dir);
    let external_media = root.join("external/deck-fixture/media/flags");
    fs::create_dir_all(&external_media).unwrap();
    fs::write(external_media.join("fi.png"), b"png").unwrap();

    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        manifest_dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);
    let media = ureq::get(&server.url("/api/media/flags/fi.png"))
        .call()
        .expect("GET declared media from inferred external media root succeeds");
    assert_eq!(media.status(), 200);
    assert_eq!(
        media.header("content-type").unwrap_or_default(),
        "image/png"
    );
}

#[test]
fn workbench_ug_detail_media_urls_serve_declared_bytes_from_media_root() {
    let media_root = temp_dir("workbench-ug-media-diagnostics-root");
    write_ug_media_diagnostics_root(&media_root);
    let manifest = workspace_root().join("fixtures/ultimate-geography/brainbrew.yaml");
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        manifest.to_str().unwrap(),
        "--media-root",
        media_root.to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let note_detail = get_json(
        &server.url("/api/workbench/note-detail?language=de&target=standard&note=note.abkhazia"),
    );
    let card_pivot = get_json(&server.url(
        "/api/workbench/card-pivot?language=de&target=standard&card=note.abkhazia%3A%3Atemplate.flag-country",
    ));
    let mut media_paths = BTreeSet::new();
    collect_api_media_paths(&note_detail, &mut media_paths);
    collect_api_media_paths(&card_pivot, &mut media_paths);

    for expected in ["ug-flag-abkhazia.svg", "ug-map-abkhazia.png"] {
        assert!(
            media_paths.contains(expected),
            "detail APIs did not expose expected media path {expected:?}; extracted paths: {media_paths:?}"
        );
    }
    assert!(
        media_paths.len() >= 2,
        "expected several unique UG media paths from detail APIs; extracted paths: {media_paths:?}"
    );

    for media_path in &media_paths {
        assert_media_response_serves_declared_bytes(&server, &media_root, media_path);
    }
}

#[test]
fn workbench_optional_metadata_progress_and_apply_are_separate_from_main_fields() {
    let dir = temp_dir("workbench-optional-metadata");
    write_workbench_optional_metadata_workspace(&dir);
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let pivot = get_json(&server.url("/api/workbench/note-pivot?language=da"));
    assert_eq!(pivot["progress"]["total"], 2);
    assert_eq!(pivot["progress"]["complete"], 2);
    assert_eq!(pivot["progress"]["stale"], 0);
    assert!(pivot["metadata_progress"]["stale"].as_u64().unwrap() >= 1);

    let optional = get_json(&server.url("/api/workbench/metadata?language=da"));
    assert_eq!(optional["main_progress"]["complete"], 2);
    let optional_items = optional["items"].as_array().unwrap();
    assert!(
        optional_items.iter().all(|item| !item["path"]
            .as_str()
            .unwrap_or_default()
            .contains(".adapter_ids.")),
        "adapter IDs are identity metadata, not optional translation metadata: {optional_items:?}"
    );
    let category_positions = optional_items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            (
                item["metadata_category_key"].as_str().unwrap_or_default(),
                index,
            )
        })
        .collect::<Vec<_>>();
    let note_type_position = category_positions
        .iter()
        .find_map(|(category, index)| (*category == "note-type-name").then_some(*index))
        .unwrap();
    let field_label_position = category_positions
        .iter()
        .find_map(|(category, index)| (*category == "field-label").then_some(*index))
        .unwrap();
    let template_position = category_positions
        .iter()
        .find_map(|(category, index)| (*category == "card-template-name").then_some(*index))
        .unwrap();
    assert!(
        note_type_position < field_label_position && field_label_position < template_position,
        "metadata should be grouped in maintainer-friendly category order: {category_positions:?}"
    );
    let deck_name = optional_items
        .iter()
        .find(|item| item["path"] == "deck.name")
        .unwrap();
    assert_eq!(deck_name["status"], "stale");
    assert!(
        deck_name["warning"]
            .as_str()
            .unwrap()
            .contains("Old Workbench")
    );

    let request = serde_json::json!({
        "language": "da",
        "edits": [{
            "kind": "translation",
            "path": "deck.name",
            "source": "Ultimate Geography",
            "value": "Arbejdsbord Røgtest",
            "mode": "direct"
        }]
    });
    let preview = post_json(&server.url("/api/workbench/apply-preview"), request.clone());
    assert_eq!(preview["validation"]["ok"], true);
    assert_eq!(preview["changed_entries"][0]["path"], "deck.name");
    assert!(
        preview["file_groups"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["file"] == "da.yaml")
    );
    assert!(
        !fs::read_to_string(dir.join("da.yaml"))
            .unwrap()
            .contains("Arbejdsbord Røgtest")
    );

    let applied = post_json(&server.url("/api/workbench/apply"), request);
    assert_eq!(applied["applied"], true);
    assert!(
        fs::read_to_string(dir.join("da.yaml"))
            .unwrap()
            .contains("Arbejdsbord Røgtest")
    );
}

#[test]
fn workbench_comparison_pane_summarizes_note_source_string_and_card_context() {
    let dir = temp_dir("workbench-comparison-pane");
    write_workbench_repeated_source_multi_language_workspace(&dir);
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let comparison = get_json(
        &server.url("/api/workbench/comparison-pane?language=nb&target=standard&overlay=base"),
    );
    assert_eq!(comparison["language"]["code"], "nb");
    assert_eq!(comparison["target_label"], "standard");
    assert!(
        comparison["content_groups"]
            .as_array()
            .unwrap()
            .iter()
            .any(|group| group == "Europe")
    );
    let strings = comparison["source_string_pivot"]["strings"]
        .as_array()
        .unwrap();
    let shared = strings
        .iter()
        .find(|string| string["source"] == "Shared capital")
        .unwrap();
    assert_eq!(shared["target_preview"], "Felles hovedstad");
    assert_eq!(shared["occurrence_count"], 2);
    assert!(comparison["card_pivot"]["cards"].as_array().unwrap().len() >= 2);
    assert!(comparison["note_pivot"]["notes"].as_array().unwrap().len() >= 2);
}

#[test]
fn workbench_apply_groups_multi_pane_edits_by_file_and_content_group() {
    let dir = temp_dir("workbench-multi-pane-apply");
    write_multi_language_workbench_workspace(&dir);
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let request = serde_json::json!({
        "language": "da",
        "target": "standard",
        "overlay": "base",
        "edits": [
            {
                "kind": "source",
                "path": "notes.note.finland.fields.field.country",
                "source": "Finland",
                "value": "Finland source",
                "scope": "field",
                "impact_action": "stale_translation"
            },
            {
                "kind": "translation",
                "language": "da",
                "target": "standard",
                "overlay": "base",
                "path": "notes.note.finland.fields.field.capital",
                "source": "Helsinki",
                "value": "Helsingfors multi",
                "mode": "direct"
            },
            {
                "kind": "translation",
                "language": "nb",
                "target": "standard",
                "overlay": "base",
                "path": "notes.note.finland.fields.field.capital",
                "source": "Helsinki",
                "value": "Helsinki norsk",
                "mode": "direct"
            }
        ]
    });
    let preview = post_json(&server.url("/api/workbench/apply-preview"), request.clone());
    assert_eq!(preview["validation"]["ok"], true);
    let affected = preview["affected_files"].as_array().unwrap();
    assert!(affected.iter().any(|file| file["path"] == "deck.yaml"));
    assert!(affected.iter().any(|file| file["path"] == "da.yaml"));
    assert!(affected.iter().any(|file| file["path"] == "nb.yaml"));
    let groups = preview["file_groups"].as_array().unwrap();
    assert!(groups.iter().any(|group| {
        group["file"] == "deck.yaml"
            && group["content_groups"]
                .as_array()
                .unwrap()
                .iter()
                .any(|content_group| content_group["name"] == "Europe")
    }));
    assert!(groups.iter().any(|group| group["file"] == "da.yaml"));
    assert!(groups.iter().any(|group| group["file"] == "nb.yaml"));
    assert!(
        !fs::read_to_string(dir.join("deck.yaml"))
            .unwrap()
            .contains("Finland source")
    );

    let applied = post_json(&server.url("/api/workbench/apply"), request);
    assert_eq!(applied["validation"]["ok"], true);
    assert!(
        fs::read_to_string(dir.join("deck.yaml"))
            .unwrap()
            .contains("field.country: Finland source")
    );
    let da = fs::read_to_string(dir.join("da.yaml")).unwrap();
    assert!(da.contains("Helsinki: Helsingfors multi"));
    assert!(da.contains("old_source: Finland"));
    assert!(da.contains("new_source: Finland source"));
    let nb = fs::read_to_string(dir.join("nb.yaml")).unwrap();
    assert!(nb.contains("Helsinki: Helsinki norsk"));
}

#[test]
fn workbench_apply_validation_failure_leaves_all_targets_unchanged() {
    let dir = temp_dir("workbench-atomic-validate");
    write_multi_language_workbench_workspace(&dir);
    let original_deck = fs::read(dir.join("deck.yaml")).unwrap();
    let original_da = fs::read(dir.join("da.yaml")).unwrap();
    let original_nb = fs::read(dir.join("nb.yaml")).unwrap();
    let server = spawn_workbench_server_with_env(
        [
            "workbench",
            "serve",
            "--manifest",
            dir.join("brainbrew.yaml").to_str().unwrap(),
            "--port",
            "0",
            "--no-open",
        ],
        &[("BRAINBREW_ATOMIC_WRITE_FAIL_VALIDATE_INDEX", "1")],
    );

    let error = post_json_error(
        &server.url("/api/workbench/apply"),
        atomic_multi_file_apply_request("atomic validation"),
    );

    assert_eq!(error.0, 500);
    assert!(
        error.1.contains("validation/serialization") || error.1.contains("validate"),
        "unexpected error body: {}",
        error.1
    );
    assert_eq!(fs::read(dir.join("deck.yaml")).unwrap(), original_deck);
    assert_eq!(fs::read(dir.join("da.yaml")).unwrap(), original_da);
    assert_eq!(fs::read(dir.join("nb.yaml")).unwrap(), original_nb);
}

#[test]
fn workbench_apply_temp_write_failure_leaves_targets_unchanged() {
    let dir = temp_dir("workbench-atomic-temp-fail");
    write_multi_language_workbench_workspace(&dir);
    let original_deck = fs::read(dir.join("deck.yaml")).unwrap();
    let original_da = fs::read(dir.join("da.yaml")).unwrap();
    let original_nb = fs::read(dir.join("nb.yaml")).unwrap();
    let server = spawn_workbench_server_with_env(
        [
            "workbench",
            "serve",
            "--manifest",
            dir.join("brainbrew.yaml").to_str().unwrap(),
            "--port",
            "0",
            "--no-open",
        ],
        &[("BRAINBREW_ATOMIC_WRITE_FAIL_TEMP_INDEX", "1")],
    );

    let error = post_json_error(
        &server.url("/api/workbench/apply"),
        atomic_multi_file_apply_request("atomic temp"),
    );

    assert_eq!(error.0, 500);
    assert!(
        error.1.contains("write temporary") || error.1.contains("temp"),
        "unexpected error body: {}",
        error.1
    );
    assert_eq!(fs::read(dir.join("deck.yaml")).unwrap(), original_deck);
    assert_eq!(fs::read(dir.join("da.yaml")).unwrap(), original_da);
    assert_eq!(fs::read(dir.join("nb.yaml")).unwrap(), original_nb);
}

#[test]
fn workbench_apply_rename_failure_reports_updated_and_not_updated_files() {
    let dir = temp_dir("workbench-atomic-rename-fail");
    write_multi_language_workbench_workspace(&dir);
    let server = spawn_workbench_server_with_env(
        [
            "workbench",
            "serve",
            "--manifest",
            dir.join("brainbrew.yaml").to_str().unwrap(),
            "--port",
            "0",
            "--no-open",
        ],
        &[("BRAINBREW_ATOMIC_WRITE_FAIL_RENAME_INDEX", "1")],
    );

    let error = post_json_error(
        &server.url("/api/workbench/apply"),
        atomic_multi_file_apply_request("atomic rename"),
    );

    assert_eq!(error.0, 500);
    assert!(error.1.contains("rename phase failed"), "{}", error.1);
    assert!(error.1.contains("updated files: deck.yaml"), "{}", error.1);
    assert!(
        error.1.contains("not updated files: da.yaml, nb.yaml")
            || error.1.contains("not updated files: nb.yaml, da.yaml"),
        "{}",
        error.1
    );
}

#[test]
fn workbench_apply_uses_atomic_temp_rename_helper() {
    let dir = temp_dir("workbench-atomic-trace");
    write_multi_language_workbench_workspace(&dir);
    let trace_path = dir.join("atomic-trace.log");
    let server = spawn_workbench_server_with_env(
        [
            "workbench",
            "serve",
            "--manifest",
            dir.join("brainbrew.yaml").to_str().unwrap(),
            "--port",
            "0",
            "--no-open",
        ],
        &[("BRAINBREW_ATOMIC_WRITE_TRACE", trace_path.to_str().unwrap())],
    );

    let applied = post_json(
        &server.url("/api/workbench/apply"),
        atomic_multi_file_apply_request("atomic trace"),
    );

    assert_eq!(applied["applied"], true);
    let trace = fs::read_to_string(trace_path).expect("atomic helper writes a trace");
    assert!(trace.contains("transaction_begin"), "trace: {trace}");
    assert!(trace.contains("temp_write"), "trace: {trace}");
    assert!(trace.contains("rename"), "trace: {trace}");
    assert!(
        !trace.contains("write_target"),
        "apply writes must not go directly to targets: {trace}"
    );
}

#[test]
fn workbench_concurrent_apply_requests_are_serialized() {
    let dir = temp_dir("workbench-atomic-concurrent");
    write_multi_language_workbench_workspace(&dir);
    let trace_path = dir.join("atomic-concurrent-trace.log");
    let trace_path_text = trace_path.to_str().unwrap().to_owned();
    let server = spawn_workbench_server_with_env(
        [
            "workbench",
            "serve",
            "--manifest",
            dir.join("brainbrew.yaml").to_str().unwrap(),
            "--port",
            "0",
            "--no-open",
        ],
        &[
            ("BRAINBREW_ATOMIC_WRITE_TRACE", trace_path_text.as_str()),
            ("BRAINBREW_ATOMIC_WRITE_SLEEP_BEFORE_RENAME_MS", "150"),
        ],
    );
    let first_url = server.url("/api/workbench/apply");
    let second_url = first_url.clone();
    let first = atomic_translation_apply_request("first serialized");
    let second = atomic_translation_apply_request("second serialized");

    let first_thread = thread::spawn(move || post_json(&first_url, first));
    let second_thread = thread::spawn(move || post_json(&second_url, second));
    assert_eq!(first_thread.join().unwrap()["applied"], true);
    assert_eq!(second_thread.join().unwrap()["applied"], true);

    let trace = fs::read_to_string(trace_path).expect("atomic helper writes a trace");
    let mut active_transactions = 0usize;
    for line in trace.lines() {
        if line.contains("transaction_begin") {
            assert_eq!(
                active_transactions, 0,
                "concurrent apply transactions overlapped: {trace}"
            );
            active_transactions += 1;
        } else if line.contains("transaction_end") {
            active_transactions = active_transactions
                .checked_sub(1)
                .expect("transaction_end without transaction_begin");
        }
    }
    assert_eq!(active_transactions, 0, "unterminated transaction: {trace}");
}

#[test]
fn workbench_card_pivot_navigates_previews_and_applies_field_edit() {
    let dir = temp_dir("workbench-card-pivot");
    write_workbench_workspace(&dir);
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let pivot = get_json(&server.url("/api/workbench/card-pivot?language=da&target=standard"));
    assert_eq!(pivot["progress"]["total"], 1);
    assert_eq!(pivot["progress"]["missing"], 1);
    assert_eq!(
        pivot["cards"][0]["card_id"],
        "note.finland::template.country-capital"
    );
    assert_eq!(pivot["cards"][0]["status"], "missing");
    assert_eq!(
        pivot["selected_card"]["template_id"],
        "template.country-capital"
    );
    assert!(
        pivot["selected_card"]["source_preview"]["cards"][0]["question_html"]
            .as_str()
            .unwrap()
            .contains("Finland")
    );
    assert!(
        pivot["selected_card"]["target_preview"]["cards"][0]["answer_html"]
            .as_str()
            .unwrap()
            .contains("Helsinki")
    );
    let capital = pivot["selected_card"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["field_id"] == "field.capital")
        .unwrap();
    assert_eq!(capital["status"], "untranslated_fallback");
    assert_eq!(capital["editable"], true);

    let filtered = get_json(&server.url(
        "/api/workbench/card-pivot?language=da&target=standard&filter=missing&content_group=Europe",
    ));
    assert_eq!(filtered["cards"].as_array().unwrap().len(), 1);
    let no_match = get_json(
        &server.url("/api/workbench/card-pivot?language=da&target=standard&content_group=Asia"),
    );
    assert_eq!(no_match["cards"].as_array().unwrap().len(), 0);

    let applied = post_json(
        &server.url("/api/workbench/apply"),
        serde_json::json!({
            "language": "da",
            "target": "standard",
            "overlay": "base",
            "edits": [{
                "path": "notes.note.finland.fields.field.capital",
                "source": "Helsinki",
                "value": "Helsingfors card",
                "mode": "direct"
            }]
        }),
    );
    assert_eq!(applied["validation"]["ok"], true);
    assert!(
        fs::read_to_string(dir.join("da.yaml"))
            .unwrap()
            .contains("Helsinki: Helsingfors card")
    );
}

#[test]
fn workbench_source_string_pivot_supports_direct_contextual_and_no_change_edits() {
    let dir = temp_dir("workbench-source-string");
    write_workbench_repeated_source_workspace(&dir);
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let pivot = get_json(&server.url(
        "/api/workbench/source-string-pivot?language=da&target=standard&source=Shared%20capital",
    ));
    let shared = pivot["strings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["source"] == "Shared capital")
        .unwrap();
    assert_eq!(shared["occurrence_count"], 2);
    assert_eq!(shared["direct_applies_to"], 2);
    assert_eq!(shared["status"], "complete");
    assert_eq!(pivot["occurrences"].as_array().unwrap().len(), 2);
    assert!(
        pivot["occurrences"][0]["friendly_context"]
            .as_str()
            .unwrap()
            .contains("Capital")
    );
    assert!(
        pivot["filters"]["content_groups"]
            .as_array()
            .unwrap()
            .iter()
            .any(|group| group == "Europe")
    );

    let preview = post_json(
        &server.url("/api/workbench/apply-preview"),
        serde_json::json!({
            "language": "da",
            "target": "standard",
            "overlay": "base",
            "edits": [{
                "kind": "translation",
                "path": "notes.note.finland.fields.field.capital",
                "source": "Shared capital",
                "value": "Fælles hovedstad opdateret",
                "mode": "direct"
            }]
        }),
    );
    assert_eq!(preview["validation"]["ok"], true);
    assert_eq!(preview["changed_entries"][0]["mode"], "direct");
    assert!(
        !fs::read_to_string(dir.join("da.yaml"))
            .unwrap()
            .contains("opdateret")
    );

    let applied = post_json(
        &server.url("/api/workbench/apply"),
        serde_json::json!({
            "language": "da",
            "target": "standard",
            "overlay": "base",
            "edits": [
                {
                    "kind": "translation",
                    "path": "notes.note.finland.fields.field.capital",
                    "source": "Shared capital",
                    "value": "Finsk særskilt",
                    "mode": "contextual",
                    "context_path": "notes.note.finland.fields.field.capital"
                },
                {
                    "kind": "translation",
                    "path": "notes.note.estonia.fields.field.country",
                    "source": "Estonia",
                    "value": "Estonia",
                    "mode": "contextual",
                    "context_path": "notes.note.estonia.fields.field.country"
                },
                {
                    "kind": "translation",
                    "path": "notes.note.finland.fields.field.country",
                    "source": "Finland",
                    "value": "Finland",
                    "mode": "no_change"
                }
            ]
        }),
    );
    assert_eq!(applied["validation"]["ok"], true);
    let overlay = fs::read_to_string(dir.join("da.yaml")).unwrap();
    assert!(overlay.contains("no_change:"));
    assert!(overlay.contains("- Finland"));
    assert!(overlay.contains("finland.fields.field.capital"));
    assert!(overlay.contains("Shared capital:"));
    assert!(overlay.contains("Finsk særskilt"));
    assert!(overlay.contains("estonia.fields.field.country"));
    assert!(overlay.contains("Estonia: Estonia"));
}

#[test]
fn workbench_source_string_pivot_exposes_structured_message_components() {
    let dir = temp_dir("workbench-source-string-structured");
    write_structured_message_translation_workspace(&dir);
    fs::write(
        dir.join("brainbrew.yaml"),
        r#"base: deck.yaml
overlays:
  overlay.translation.nb:
    file: nb.yaml
    kind: translation
targets:
  nb-standard:
    overlays:
      - overlay.translation.nb
  en-standard:
    overlays: []
languages:
  nb:
    display_name: Norwegian Bokmål
    translation_overlays:
      base: overlay.translation.nb
    primary_target: standard
    targets:
      standard: nb-standard
  en:
    display_name: English
    source: true
    primary_target: standard
    targets:
      standard: en-standard
translation_profile:
  structural_fields:
    - field.flag
  metadata_categories:
    - key: deck-metadata
      label: Deck metadata
      paths:
        - deck.name
        - deck.description
  metadata_paths:
    - deck.*
"#,
    )
    .unwrap();
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let pivot = get_json(&server.url(
        "/api/workbench/source-string-pivot?language=nb&target=standard&source=blue%20background%20with%20a%20white%20cross",
    ));
    assert_eq!(
        pivot["selected_source"],
        "blue background with a white cross"
    );
    assert_eq!(pivot["occurrences"].as_array().unwrap().len(), 1);
    let occurrence = &pivot["occurrences"][0];
    assert_eq!(
        occurrence["path"],
        "notes.note.finland.fields.field.flag-similarity.message.2"
    );
    assert_eq!(occurrence["target"], "blå bakgrunn med hvitt kors");

    let applied = post_json(
        &server.url("/api/workbench/apply"),
        serde_json::json!({
            "language": "nb",
            "target": "standard",
            "overlay": "base",
            "edits": [{
                "kind": "translation",
                "path": "notes.note.finland.fields.field.flag-similarity.message.2",
                "source": "blue background with a white cross",
                "value": "blå bakgrunn med kvitt kors",
                "mode": "direct"
            }]
        }),
    );
    assert_eq!(applied["validation"]["ok"], true);
    let overlay = fs::read_to_string(dir.join("nb.yaml")).unwrap();
    assert!(overlay.contains("blue background with a white cross:"));
    assert!(overlay.contains("blå bakgrunn med kvitt kors"));
}

#[test]
fn workbench_apply_preview_and_apply_write_translation_overlay() {
    let dir = temp_dir("workbench-apply");
    write_workbench_workspace(&dir);
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);
    let request = serde_json::json!({
        "language": "da",
        "target": "standard",
        "overlay": "base",
        "edits": [{
            "path": "notes.note.finland.fields.field.capital",
            "source": "Helsinki",
            "value": "Helsingfors",
            "mode": "direct"
        }]
    });

    let rejected = ureq::post(&server.url("/api/workbench/apply-preview"))
        .set("content-type", "application/json")
        .send_string(
            &serde_json::json!({
                "language": "da",
                "target": "standard",
                "overlay": "base",
                "edits": [{
                    "path": "notes.note.finland.fields.field.capital",
                    "source": "Bogus",
                    "value": "Stale",
                    "mode": "direct"
                }]
            })
            .to_string(),
        )
        .expect_err("stale staged source is rejected");
    match rejected {
        ureq::Error::Status(status, response) => {
            assert_eq!(status, 400);
            assert!(response.into_string().unwrap().contains("invalid source"));
        }
        other => panic!("unexpected stale-source error: {other}"),
    }

    let contextual_preview = post_json(
        &server.url("/api/workbench/apply-preview"),
        serde_json::json!({
            "language": "da",
            "target": "standard",
            "overlay": "base",
            "edits": [{
                "path": "notes.note.finland.fields.field.capital",
                "source": "Helsinki",
                "value": "Helsingfors",
                "mode": "contextual"
            }]
        }),
    );
    assert_eq!(contextual_preview["validation"]["ok"], true);
    assert_eq!(
        contextual_preview["changed_entries"][0]["path"],
        "notes.note.finland"
    );
    assert_eq!(
        contextual_preview["changed_entries"][0]["field_path"],
        "notes.note.finland.fields.field.capital"
    );
    assert!(
        !fs::read_to_string(dir.join("da.yaml"))
            .unwrap()
            .contains("contextual")
    );

    let preview = post_json(&server.url("/api/workbench/apply-preview"), request.clone());
    assert_eq!(preview["mode"], "preview");
    assert_eq!(preview["applied"], false);
    assert_eq!(preview["validation"]["ok"], true);
    assert_eq!(preview["affected_files"][0]["path"], "da.yaml");
    assert_eq!(preview["changed_entries"][0]["source"], "Helsinki");
    assert!(
        !fs::read_to_string(dir.join("da.yaml"))
            .unwrap()
            .contains("Helsingfors")
    );

    let applied = post_json(&server.url("/api/workbench/apply"), request);
    assert_eq!(applied["mode"], "write");
    assert_eq!(applied["applied"], true);
    let overlay = fs::read_to_string(dir.join("da.yaml")).unwrap();
    assert!(overlay.contains("Helsinki: Helsingfors"));

    let pivot = get_json(&server.url("/api/workbench/note-pivot?language=da&target=standard"));
    assert_eq!(pivot["progress"]["complete"], 2);
    let capital = pivot["notes"][0]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["field_id"] == "field.capital")
        .unwrap()
        .clone();
    assert_eq!(capital["target"], "Helsingfors");
    assert_eq!(capital["status"], "direct_translation");
}

#[test]
fn workbench_source_edits_create_contextual_stale_translations_for_changed_occurrence() {
    let dir = temp_dir("workbench-source-stale");
    write_workbench_repeated_source_workspace(&dir);
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let request = serde_json::json!({
        "language": "da",
        "target": "standard",
        "overlay": "base",
        "edits": [{
            "kind": "source",
            "path": "notes.note.finland.fields.field.capital",
            "source": "Shared capital",
            "value": "Finnish capital",
            "scope": "field",
            "impact_action": "stale_translation"
        }]
    });

    let preview = post_json(&server.url("/api/workbench/apply-preview"), request.clone());
    assert_eq!(preview["mode"], "preview");
    assert_eq!(preview["applied"], false);
    assert_eq!(preview["validation"]["ok"], true);
    assert!(
        preview["affected_files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["path"] == "deck.yaml")
    );
    assert!(
        preview["affected_files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["path"] == "da.yaml")
    );
    assert!(
        preview["changed_entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| {
                entry["mode"] == "stale_translation"
                    && entry["old_source"] == "Shared capital"
                    && entry["new_source"] == "Finnish capital"
                    && entry["target"] == "Fælles hovedstad"
                    && entry["context"] == "notes.note.finland"
            })
    );
    assert!(
        !fs::read_to_string(dir.join("deck.yaml"))
            .unwrap()
            .contains("Finnish capital")
    );
    assert!(
        !fs::read_to_string(dir.join("da.yaml"))
            .unwrap()
            .contains("stale_translations")
    );

    let applied = post_json(&server.url("/api/workbench/apply"), request);
    assert_eq!(applied["mode"], "write");
    assert_eq!(applied["applied"], true);
    let deck = fs::read_to_string(dir.join("deck.yaml")).unwrap();
    assert!(deck.contains("field.capital: Finnish capital"));
    assert!(deck.contains("field.capital: Shared capital"));
    let overlay = fs::read_to_string(dir.join("da.yaml")).unwrap();
    assert!(overlay.contains("Shared capital:"));
    assert!(overlay.contains("Fælles hovedstad"));
    assert!(overlay.contains("stale_translations:"));
    assert!(overlay.contains("old_source: Shared capital"));
    assert!(overlay.contains("new_source: Finnish capital"));
    assert!(overlay.contains("target:"));
    assert!(overlay.contains("context: notes.note.finland"));

    let pivot = get_json(&server.url("/api/workbench/note-pivot?language=da&target=standard"));
    let finland_capital = pivot["notes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|note| note["note_id"] == "note.finland")
        .unwrap()["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["field_id"] == "field.capital")
        .unwrap()
        .clone();
    assert_eq!(finland_capital["source"], "Finnish capital");
    assert_eq!(finland_capital["target"], "Fælles hovedstad");
    assert_eq!(finland_capital["status"], "stale_translation");
    let estonia_capital = pivot["notes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|note| note["note_id"] == "note.estonia")
        .unwrap()["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["field_id"] == "field.capital")
        .unwrap()
        .clone();
    assert_eq!(estonia_capital["source"], "Shared capital");
    assert_eq!(estonia_capital["status"], "direct_translation");
}

#[test]
fn workbench_source_edits_preserve_contextual_impacts_per_occurrence() {
    let dir = temp_dir("workbench-source-contextual-stale");
    write_workbench_repeated_source_contextual_workspace(&dir);
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);
    let request = serde_json::json!({
        "language": "da",
        "target": "standard",
        "overlay": "base",
        "edits": [{
            "kind": "source",
            "path": "notes.note.finland.fields.field.capital",
            "source": "Shared capital",
            "value": "Regional capital",
            "scope": "all_occurrences",
            "impact_action": "stale_translation"
        }]
    });

    let applied = post_json(&server.url("/api/workbench/apply"), request);
    assert_eq!(applied["validation"]["ok"], true);
    let stale_entries = applied["changed_entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["mode"] == "stale_translation")
        .collect::<Vec<_>>();
    assert_eq!(stale_entries.len(), 2);
    assert!(stale_entries.iter().any(|entry| {
        entry["context"] == "notes.note.finland" && entry["target"] == "Finsk fælles"
    }));
    assert!(stale_entries.iter().any(|entry| {
        entry["context"] == "notes.note.estonia" && entry["target"] == "Estisk fælles"
    }));
    let overlay = fs::read_to_string(dir.join("da.yaml")).unwrap();
    assert!(overlay.contains("stale_translations:"));
    assert!(overlay.contains("Finsk fælles"));
    assert!(overlay.contains("Estisk fælles"));
    assert!(!overlay.contains("contextual:"));

    let dir = temp_dir("workbench-source-contextual-migrate");
    write_workbench_repeated_source_contextual_workspace(&dir);
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);
    let request = serde_json::json!({
        "language": "da",
        "target": "standard",
        "overlay": "base",
        "edits": [{
            "kind": "source",
            "path": "notes.note.finland.fields.field.capital",
            "source": "Shared capital",
            "value": "Regional capital",
            "scope": "all_occurrences",
            "impact_action": "migrate_key"
        }]
    });

    let applied = post_json(&server.url("/api/workbench/apply"), request);
    assert_eq!(applied["validation"]["ok"], true);
    let migrated_entries = applied["changed_entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["mode"] == "migrate_key")
        .collect::<Vec<_>>();
    assert_eq!(migrated_entries.len(), 2);
    let overlay = fs::read_to_string(dir.join("da.yaml")).unwrap();
    assert!(overlay.contains("Regional capital:"));
    assert!(overlay.contains("Finsk fælles"));
    assert!(overlay.contains("Estisk fælles"));
    assert!(!overlay.contains("Shared capital:"));
    assert!(!overlay.contains("stale_translations"));
}

#[test]
fn workbench_source_edits_can_migrate_keys_change_all_and_preserve_includes() {
    let dir = temp_dir("workbench-source-migrate");
    write_workbench_repeated_source_workspace(&dir);
    fs::create_dir_all(dir.join("content")).unwrap();
    fs::write(dir.join("content/finland-capital.txt"), "Shared capital").unwrap();
    let deck = fs::read_to_string(dir.join("deck.yaml")).unwrap().replacen(
        "field.capital: Shared capital",
        "field.capital: !include content/finland-capital.txt",
        1,
    );
    fs::write(dir.join("deck.yaml"), deck).unwrap();
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let request = serde_json::json!({
        "language": "da",
        "target": "standard",
        "overlay": "base",
        "edits": [{
            "kind": "source",
            "path": "notes.note.finland.fields.field.capital",
            "source": "Shared capital",
            "value": "Migrated capital",
            "scope": "all_occurrences",
            "impact_action": "migrate_key"
        }]
    });

    let applied = post_json(&server.url("/api/workbench/apply"), request);
    assert_eq!(applied["validation"]["ok"], true);
    let deck = fs::read_to_string(dir.join("deck.yaml")).unwrap();
    assert!(deck.contains("field.capital: !include content/finland-capital.txt"));
    assert!(deck.contains("field.capital: Migrated capital"));
    assert_eq!(
        fs::read_to_string(dir.join("content/finland-capital.txt")).unwrap(),
        "Migrated capital"
    );
    let overlay = fs::read_to_string(dir.join("da.yaml")).unwrap();
    assert!(overlay.contains("Migrated capital:"));
    assert!(overlay.contains("Fælles hovedstad"));
    assert!(!overlay.contains("Shared capital:"));
    assert!(!overlay.contains("stale_translations"));
}

#[test]
fn workbench_mixed_source_then_translation_apply_uses_new_source_state() {
    let dir = temp_dir("workbench-source-mixed");
    write_workbench_repeated_source_workspace(&dir);
    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
    ]);

    let request = serde_json::json!({
        "language": "da",
        "target": "standard",
        "overlay": "base",
        "edits": [
            {
                "kind": "source",
                "path": "notes.note.finland.fields.field.capital",
                "source": "Shared capital",
                "value": "Finnish capital",
                "scope": "field",
                "impact_action": "stale_translation"
            },
            {
                "kind": "translation",
                "path": "notes.note.finland.fields.field.capital",
                "source": "Finnish capital",
                "value": "Finsk hovedstad",
                "mode": "contextual"
            }
        ]
    });

    let applied = post_json(&server.url("/api/workbench/apply"), request);
    assert_eq!(applied["validation"]["ok"], true);
    let overlay = fs::read_to_string(dir.join("da.yaml")).unwrap();
    assert!(overlay.contains("Shared capital:"));
    assert!(overlay.contains("Fælles hovedstad"));
    assert!(overlay.contains("Finnish capital:"));
    assert!(overlay.contains("Finsk hovedstad"));
    assert!(!overlay.contains("stale_translations"));
}

#[test]
fn workbench_serve_can_use_dev_asset_directory() {
    let dir = temp_dir("workbench-dev-assets");
    write_workbench_workspace(&dir);
    let assets = dir.join("assets");
    fs::create_dir_all(&assets).unwrap();
    fs::write(
        assets.join("index.html"),
        "<main>dev workbench shell</main>",
    )
    .unwrap();

    let server = spawn_workbench_server([
        "workbench",
        "serve",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--port",
        "0",
        "--no-open",
        "--dev-assets",
        assets.to_str().unwrap(),
    ]);

    let response = ureq::get(&server.url("/")).call().expect("GET / succeeds");
    assert_eq!(response.status(), 200);
    assert!(
        response
            .into_string()
            .unwrap()
            .contains("dev workbench shell")
    );
}

#[test]
fn validate_without_args_shows_usage_examples() {
    let output = run(["validate"]);

    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("Usage:"));
    assert!(err.contains("Examples:"));
    assert!(err.contains("brainbrew validate deck.yaml"));
}

#[test]
fn validate_reports_valid_deck_human_readably() {
    let dir = temp_dir("validate-valid");
    let deck_path = dir.join("deck.yaml");
    fs::write(&deck_path, SAMPLE_CANONICAL_YAML).unwrap();

    let output = run(["validate", deck_path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("✓"));
    assert!(out.contains("valid deck"));
    assert!(out.contains(deck_path.to_str().unwrap()));
    assert!(out.contains("notes: 1"));
}

#[test]
fn validate_reports_invalid_deck_path() {
    let dir = temp_dir("validate-invalid");
    let deck_path = dir.join("deck.yaml");
    fs::write(
        &deck_path,
        SAMPLE_CANONICAL_YAML.replace(
            "note_type_id: note-type.country",
            "note_type_id: note-type.missing",
        ),
    )
    .unwrap();

    let output = run(["validate", deck_path.to_str().unwrap()]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("notes.note.finland.note_type_id"));
}

#[test]
fn fmt_rewrites_canonical_yaml_in_place() {
    let dir = temp_dir("fmt");
    let deck_path = dir.join("deck.yaml");
    fs::write(&deck_path, MESSY_CANONICAL_YAML).unwrap();

    let output = run(["fmt", deck_path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(
        fs::read_to_string(deck_path).unwrap(),
        SAMPLE_CANONICAL_YAML
    );
}

#[test]
fn fmt_rewrites_overlay_yaml_in_place() {
    let dir = temp_dir("fmt-overlay");
    let overlay_path = dir.join("overlay.yaml");
    fs::write(&overlay_path, MESSY_OVERLAY_YAML).unwrap();

    let output = run(["fmt", overlay_path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(
        fs::read_to_string(overlay_path).unwrap(),
        CAPITAL_OVERLAY_YAML
    );
}

#[test]
fn fmt_rewrites_manifest_yaml_in_place() {
    let dir = temp_dir("fmt-manifest");
    let manifest_path = dir.join("brainbrew.yaml");
    fs::write(&manifest_path, MESSY_MANIFEST_YAML).unwrap();

    let output = run(["fmt", manifest_path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(fs::read_to_string(manifest_path).unwrap(), MANIFEST_YAML);
}

#[test]
fn fmt_rewrites_federation_lock_yaml_in_place() {
    let dir = temp_dir("fmt-lock");
    let lock_path = dir.join("brainbrew.lock");
    fs::write(&lock_path, MESSY_LOCK_YAML).unwrap();

    let output = run(["fmt", lock_path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(fs::read_to_string(lock_path).unwrap(), LOCK_YAML);
}

#[test]
fn compose_applies_overlay_files_in_order() {
    let dir = temp_dir("compose-overlay");
    let deck_path = dir.join("deck.yaml");
    let overlay_path = dir.join("overlay.yaml");
    let resolved_path = dir.join("resolved.yaml");
    fs::write(&deck_path, SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(&overlay_path, CAPITAL_OVERLAY_YAML).unwrap();

    let output = run([
        "compose",
        deck_path.to_str().unwrap(),
        "--overlay",
        overlay_path.to_str().unwrap(),
        "--out",
        resolved_path.to_str().unwrap(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("✓"));
    assert!(out.contains("composed deck"));
    assert!(out.contains(resolved_path.to_str().unwrap()));
    assert!(
        fs::read_to_string(resolved_path)
            .unwrap()
            .contains("field.capital: Helsingfors")
    );
}

#[test]
fn targets_lists_manifest_targets() {
    let dir = temp_dir("targets-manifest");
    write_manifest_workspace(&dir);

    let output = run([
        "targets",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "patched-via-dependency\n");
}

#[test]
fn targets_can_discover_multiple_package_manifests() {
    let first = temp_dir("targets-package-first");
    let second = temp_dir("targets-package-second");
    write_manifest_workspace(&first);
    write_manifest_workspace(&second);
    fs::write(first.join("brainbrew.yaml"), MANIFEST_WITH_PACKAGE_YAML).unwrap();
    fs::write(
        second.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML
            .replace("anki-geo.ultimate-geography", "anki-geo.rivers")
            .replace("patched-via-dependency", "rivers"),
    )
    .unwrap();

    let output = run([
        "targets",
        "--manifest",
        first.join("brainbrew.yaml").to_str().unwrap(),
        "--include",
        second.join("brainbrew.yaml").to_str().unwrap(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("anki-geo.ultimate-geography:patched-via-dependency"));
    assert!(out.contains("anki-geo.rivers:rivers"));
}

#[test]
fn targets_discovers_package_root_and_validates_dependencies() {
    let root = temp_dir("targets-package-root");
    let ug = root.join("ultimate-geography");
    let rivers = root.join("rivers");
    fs::create_dir_all(&ug).unwrap();
    fs::create_dir_all(&rivers).unwrap();
    write_manifest_workspace(&ug);
    write_manifest_workspace(&rivers);
    fs::write(
        ug.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML.replace("  depends_on:\n    - anki-geo.shared-geography\n", ""),
    )
    .unwrap();
    fs::write(
        rivers.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML
            .replace("anki-geo.ultimate-geography", "anki-geo.rivers")
            .replace(
                "depends_on:\n    - anki-geo.shared-geography",
                "depends_on:\n    - anki-geo.ultimate-geography@0.1.0",
            )
            .replace("patched-via-dependency", "rivers"),
    )
    .unwrap();

    let output = run(["targets", "--package-root", root.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("anki-geo.ultimate-geography:patched-via-dependency"));
    assert!(out.contains("anki-geo.rivers:rivers"));
}

#[test]
fn compose_can_resolve_extended_targets_from_brainbrew_lock() {
    let root = temp_dir("compose-federated-lock");
    let ug = root.join("ultimate-geography");
    let america = root.join("america");
    fs::create_dir_all(&ug).unwrap();
    fs::create_dir_all(&america).unwrap();
    write_manifest_workspace(&ug);
    fs::write(
        ug.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML.replace("  depends_on:\n    - anki-geo.shared-geography\n", ""),
    )
    .unwrap();
    fs::write(america.join("deck.yaml"), SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        america.join("america.yaml"),
        r#"id: overlay.extension.america
kind: extension
notes:
  note.finland:
    intent: merge
    tags:
      America::Imported:
        intent: add
"#,
    )
    .unwrap();
    fs::write(
        america.join("brainbrew.yaml"),
        r#"package:
  id: anki-geo.america
  version: 0.1.0
  depends_on:
    - anki-geo.ultimate-geography@0.1.0
base: deck.yaml
overlays:
  overlay.extension.america:
    file: america.yaml
    kind: extension
targets:
  en-america:
    extends: anki-geo.ultimate-geography:patched-via-dependency
    overlays:
      - overlay.extension.america
"#,
    )
    .unwrap();
    fs::write(
        america.join("brainbrew.lock"),
        format!(
            r#"version: 1
packages:
  anki-geo.ultimate-geography:
    manifest: brainbrew.yaml
    package:
      version: 0.1.0
    locked:
      type: path
      path: '{}'
"#,
            ug.canonicalize().unwrap().display()
        ),
    )
    .unwrap();
    let resolved = root.join("resolved.yaml");
    let cache = root.join("cache");

    let output = run_with_cache(
        [
            "compose",
            "--manifest",
            america.join("brainbrew.yaml").to_str().unwrap(),
            "--target",
            "en-america",
            "--out",
            resolved.to_str().unwrap(),
        ],
        &cache,
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let resolved_source = fs::read_to_string(resolved).unwrap();
    assert!(resolved_source.contains("field.capital: Helsingfors"));
    assert!(resolved_source.contains("America::Imported"));
}

#[test]
fn lock_update_and_verify_path_package_without_nix() {
    let root = temp_dir("lock-update-path");
    let ug = root.join("ultimate-geography");
    let america = root.join("america");
    fs::create_dir_all(&ug).unwrap();
    fs::create_dir_all(&america).unwrap();
    write_manifest_workspace(&ug);
    fs::write(
        ug.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML.replace("  depends_on:\n    - anki-geo.shared-geography\n", ""),
    )
    .unwrap();
    let lock_path = america.join("brainbrew.lock");

    let cache = root.join("cache");
    let update = run_with_cache(
        [
            "lock",
            "update",
            "--lock",
            lock_path.to_str().unwrap(),
            "--package",
            "anki-geo.ultimate-geography",
            "--path",
            ug.to_str().unwrap(),
        ],
        &cache,
    );

    assert!(update.status.success(), "stderr: {}", stderr(&update));
    let lock_source = fs::read_to_string(&lock_path).unwrap();
    assert!(lock_source.contains("original:\n      type: path"));
    assert!(lock_source.contains("locked:\n      type: path"));
    assert!(lock_source.contains(&format!("path: {}", ug.canonicalize().unwrap().display())));
    assert!(!lock_source.contains("/nix/store/"));
    assert!(lock_source.contains("nar_hash: 'sha256-"));

    let verify = run_with_cache(
        ["lock", "verify", "--lock", lock_path.to_str().unwrap()],
        &cache,
    );

    assert!(verify.status.success(), "stderr: {}", stderr(&verify));
    assert!(stdout(&verify).contains("verified 1 locked package"));

    fs::write(
        &lock_path,
        fs::read_to_string(&lock_path)
            .unwrap()
            .replace("sha256-", "sha256-bad"),
    )
    .unwrap();
    let mismatch = run_with_cache(
        ["lock", "verify", "--lock", lock_path.to_str().unwrap()],
        &cache,
    );

    assert!(!mismatch.status.success());
    assert!(stderr(&mismatch).contains("nar_hash mismatch"));
}

#[test]
fn lock_update_and_verify_tarball_package_without_nix() {
    let root = temp_dir("lock-update-tarball");
    let ug = root.join("ultimate-geography");
    let america = root.join("america");
    fs::create_dir_all(&ug).unwrap();
    fs::create_dir_all(&america).unwrap();
    write_manifest_workspace(&ug);
    fs::write(
        ug.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML.replace("  depends_on:\n    - anki-geo.shared-geography\n", ""),
    )
    .unwrap();
    let archive_path = root.join("ultimate-geography.tar.gz");
    write_tar_gz(&archive_path, "ultimate-geography", &ug);
    let lock_path = america.join("brainbrew.lock");
    let cache = root.join("cache");

    let update = run_with_cache(
        [
            "lock",
            "update",
            "--lock",
            lock_path.to_str().unwrap(),
            "--package",
            "anki-geo.ultimate-geography",
            "--tarball",
            &format!("file://{}", archive_path.display()),
        ],
        &cache,
    );

    assert!(update.status.success(), "stderr: {}", stderr(&update));
    let lock_source = fs::read_to_string(&lock_path).unwrap();
    assert!(lock_source.contains("original:\n      type: tarball"));
    assert!(lock_source.contains("locked:\n      type: tarball"));
    assert!(lock_source.contains("nar_hash: 'sha256-"));

    let verify = run_with_cache(
        ["lock", "verify", "--lock", lock_path.to_str().unwrap()],
        &cache,
    );

    assert!(verify.status.success(), "stderr: {}", stderr(&verify));
    assert!(stdout(&verify).contains("verified 1 locked package"));
}

#[test]
fn compose_can_extend_targets_and_mix_overlays_from_included_package_manifests() {
    let root = temp_dir("compose-federated-package");
    let ug = root.join("ultimate-geography");
    let america = root.join("america");
    fs::create_dir_all(&ug).unwrap();
    fs::create_dir_all(&america).unwrap();
    write_manifest_workspace(&ug);
    fs::write(
        ug.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML.replace("  depends_on:\n    - anki-geo.shared-geography\n", ""),
    )
    .unwrap();
    fs::write(america.join("deck.yaml"), SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        america.join("america.yaml"),
        r#"id: overlay.extension.america
kind: extension
notes:
  note.finland:
    intent: merge
    tags:
      America::Imported:
        intent: add
"#,
    )
    .unwrap();
    fs::write(
        america.join("brainbrew.yaml"),
        r#"package:
  id: anki-geo.america
  version: 0.1.0
  depends_on:
    - anki-geo.ultimate-geography@0.1.0
base: deck.yaml
overlays:
  overlay.extension.america:
    file: america.yaml
    kind: extension
targets:
  en-america:
    extends: anki-geo.ultimate-geography:patched-via-dependency
    overlays:
      - overlay.extension.america
"#,
    )
    .unwrap();
    let america_manifest = america.join("brainbrew.yaml");
    let ug_manifest = ug.join("brainbrew.yaml");
    let resolved = root.join("resolved.yaml");

    let output = run([
        "compose",
        "--manifest",
        america_manifest.to_str().unwrap(),
        "--include",
        ug_manifest.to_str().unwrap(),
        "--target",
        "en-america",
        "--out",
        resolved.to_str().unwrap(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let resolved_source = fs::read_to_string(resolved).unwrap();
    assert!(resolved_source.contains("field.capital: Helsingfors"));
    assert!(resolved_source.contains("America::Imported"));

    let mixer = root.join("mixer");
    fs::create_dir_all(&mixer).unwrap();
    fs::write(mixer.join("deck.yaml"), SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        mixer.join("brainbrew.yaml"),
        r#"package:
  id: example.mix
  version: 0.1.0
  depends_on:
    - anki-geo.ultimate-geography@0.1.0
    - anki-geo.america@0.1.0
base: deck.yaml
overlays: {}
targets:
  en-mixed:
    extends: anki-geo.ultimate-geography:patched-via-dependency
    overlays:
      - anki-geo.america:overlay.extension.america
"#,
    )
    .unwrap();
    let mixer_manifest = mixer.join("brainbrew.yaml");
    let mixed_resolved = root.join("mixed-resolved.yaml");

    let output = run([
        "compose",
        "--manifest",
        mixer_manifest.to_str().unwrap(),
        "--include",
        ug_manifest.to_str().unwrap(),
        "--include",
        america_manifest.to_str().unwrap(),
        "--target",
        "en-mixed",
        "--out",
        mixed_resolved.to_str().unwrap(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let mixed_source = fs::read_to_string(mixed_resolved).unwrap();
    assert!(mixed_source.contains("field.capital: Helsingfors"));
    assert!(mixed_source.contains("America::Imported"));
}

#[test]
fn targets_reports_missing_package_dependencies() {
    let root = temp_dir("targets-missing-package-dep");
    let rivers = root.join("rivers");
    fs::create_dir_all(&rivers).unwrap();
    write_manifest_workspace(&rivers);
    fs::write(
        rivers.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML
            .replace("anki-geo.ultimate-geography", "anki-geo.rivers")
            .replace(
                "depends_on:\n    - anki-geo.shared-geography",
                "depends_on:\n    - anki-geo.ultimate-geography@0.1.0",
            )
            .replace("patched-via-dependency", "rivers"),
    )
    .unwrap();

    let output = run(["targets", "--package-root", root.to_str().unwrap()]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("package dependency anki-geo.ultimate-geography"));
}

#[test]
fn targets_reports_package_dependency_version_mismatches() {
    let root = temp_dir("targets-package-version-mismatch");
    let ug = root.join("ultimate-geography");
    let rivers = root.join("rivers");
    fs::create_dir_all(&ug).unwrap();
    fs::create_dir_all(&rivers).unwrap();
    write_manifest_workspace(&ug);
    write_manifest_workspace(&rivers);
    fs::write(
        ug.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML.replace("  depends_on:\n    - anki-geo.shared-geography\n", ""),
    )
    .unwrap();
    fs::write(
        rivers.join("brainbrew.yaml"),
        MANIFEST_WITH_PACKAGE_YAML
            .replace("anki-geo.ultimate-geography", "anki-geo.rivers")
            .replace(
                "depends_on:\n    - anki-geo.shared-geography",
                "depends_on:\n    - anki-geo.ultimate-geography@9.9.9",
            )
            .replace("patched-via-dependency", "rivers"),
    )
    .unwrap();

    let output = run(["targets", "--package-root", root.to_str().unwrap()]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("resolved to version 0.1.0"));
}

#[test]
fn targets_json_includes_package_metadata() {
    let dir = temp_dir("targets-package-json");
    write_manifest_workspace(&dir);
    fs::write(dir.join("brainbrew.yaml"), MANIFEST_WITH_PACKAGE_YAML).unwrap();

    let output = run([
        "targets",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--json",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(json["package"]["id"], "anki-geo.ultimate-geography");
    assert_eq!(json["package"]["version"], "0.1.0");
}

#[test]
fn targets_can_report_json_with_expanded_overlays() {
    let dir = temp_dir("targets-json");
    write_manifest_workspace(&dir);

    let output = run([
        "targets",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--json",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(json["targets"][0]["name"], "patched-via-dependency");
    assert_eq!(json["targets"][0]["overlays"][0]["id"], "patch.capital");
    assert_eq!(
        json["targets"][0]["overlays"][1]["id"],
        "noop.after-capital"
    );
}

#[test]
fn validate_uses_manifest_target() {
    let dir = temp_dir("validate-manifest");
    write_manifest_workspace(&dir);

    let output = run([
        "validate",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("✓"));
    assert!(out.contains("valid target"));
    assert!(out.contains("patched-via-dependency"));
}

#[test]
fn manifest_target_errors_list_available_targets() {
    let dir = temp_dir("missing-target");
    write_manifest_workspace(&dir);

    let output = run([
        "compose",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "missing",
    ]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("available targets: patched-via-dependency"));
}

#[test]
fn compose_uses_manifest_target_dependency_expansion() {
    let dir = temp_dir("compose-manifest");
    write_manifest_workspace(&dir);
    let resolved_path = dir.join("resolved.yaml");

    let output = run([
        "compose",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
        "--out",
        resolved_path.to_str().unwrap(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("✓"));
    assert!(out.contains("composed target"));
    assert!(out.contains("patched-via-dependency"));
    assert!(out.contains(resolved_path.to_str().unwrap()));
    assert!(
        fs::read_to_string(resolved_path)
            .unwrap()
            .contains("field.capital: Helsingfors")
    );
}

#[test]
fn export_crowdanki_uses_manifest_target_configured_out() {
    let dir = temp_dir("export-manifest-configured-out");
    write_manifest_workspace(&dir);
    fs::write(dir.join("brainbrew.yaml"), MANIFEST_WITH_EXPORTS_YAML).unwrap();

    let output = run([
        "export",
        "crowdanki",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(dir.join("configured-crowdanki/deck.json").exists());
}

#[test]
fn export_crowdanki_defaults_manifest_target_out_to_build_crowdanki_target() {
    let dir = temp_dir("export-manifest-default-out");
    write_manifest_workspace(&dir);

    let output = run([
        "export",
        "crowdanki",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(
        dir.join("build/crowdanki/patched-via-dependency/deck.json")
            .exists()
    );
}

#[test]
fn export_crowdanki_uses_manifest_target() {
    let dir = temp_dir("export-manifest");
    write_manifest_workspace(&dir);
    let export_dir = dir.join("crowdanki");

    let output = run([
        "export",
        "crowdanki",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
        "--out",
        export_dir.to_str().unwrap(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(
        fs::read_to_string(export_dir.join("deck.json"))
            .unwrap()
            .contains("Helsingfors")
    );
}

#[test]
fn verify_compares_configured_crowdanki_golden() {
    let dir = temp_dir("verify-golden");
    write_manifest_workspace(&dir);
    fs::write(dir.join("brainbrew.yaml"), MANIFEST_WITH_EXPORTS_YAML).unwrap();
    let golden_dir = dir.join("goldens/patched");

    let export_output = run([
        "export",
        "crowdanki",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
        "--out",
        golden_dir.to_str().unwrap(),
    ]);
    assert!(
        export_output.status.success(),
        "stderr: {}",
        stderr(&export_output)
    );

    let verify_output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
    ]);
    assert!(
        verify_output.status.success(),
        "stderr: {}",
        stderr(&verify_output)
    );

    let golden_path = golden_dir.join("deck.json");
    fs::write(
        &golden_path,
        fs::read_to_string(&golden_path)
            .unwrap()
            .replace("Helsingfors", "Helsinki"),
    )
    .unwrap();
    let mismatch_output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
    ]);
    assert!(!mismatch_output.status.success());
    assert!(stderr(&mismatch_output).contains("CrowdAnki golden mismatch"));
}

#[test]
fn verify_allows_configured_crowdanki_golden_paths() {
    let dir = temp_dir("verify-golden-allowlist");
    write_manifest_workspace(&dir);
    fs::write(
        dir.join("brainbrew.yaml"),
        MANIFEST_WITH_EXPORTS_YAML.replace(
            "        golden: goldens/patched/deck.json\n",
            "        golden: goldens/patched/deck.json\n        golden_allowlist:\n          - '$.name'\n",
        ),
    )
    .unwrap();
    let golden_dir = dir.join("goldens/patched");

    let export_output = run([
        "export",
        "crowdanki",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
        "--out",
        golden_dir.to_str().unwrap(),
    ]);
    assert!(
        export_output.status.success(),
        "stderr: {}",
        stderr(&export_output)
    );

    let golden_path = golden_dir.join("deck.json");
    let mut golden_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&golden_path).unwrap()).unwrap();
    golden_json["name"] = serde_json::json!("Allowed Legacy Name");
    fs::write(
        &golden_path,
        serde_json::to_string_pretty(&golden_json).unwrap(),
    )
    .unwrap();

    let verify_output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
    ]);
    assert!(
        verify_output.status.success(),
        "stderr: {}",
        stderr(&verify_output)
    );
}

#[test]
fn verify_checks_all_manifest_targets() {
    let dir = temp_dir("verify-manifest");
    write_manifest_workspace(&dir);

    let output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("✓"));
    assert!(out.contains("verified 1 target"));
}

#[test]
fn translate_aliases_run_translation_reports() {
    let dir = temp_dir("translate-aliases");
    write_translation_workspace(&dir);

    for command in ["translate", "translation"] {
        let output = run([
            command,
            "--manifest",
            dir.join("brainbrew.yaml").to_str().unwrap(),
            "--target",
            "da-standard",
            "--json",
        ]);

        assert!(
            output.status.success(),
            "{command} stderr: {}",
            stderr(&output)
        );
        let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
        assert_eq!(json["reports"][0]["target"], "da-standard");
    }
}

#[test]
fn unknown_translate_like_command_suggests_translations() {
    let output = run(["translatons"]);

    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("unknown command"));
    assert!(err.contains("Did you mean:"));
    assert!(err.contains("brainbrew translations"));
}

#[test]
fn translations_missing_manifest_lists_nearby_manifests() {
    let dir = temp_dir("translations-missing-manifest");
    let workspace = dir.join("decks/sample");
    fs::create_dir_all(&workspace).unwrap();
    write_translation_workspace(&workspace);

    let output = run_in_dir(["translations", "--no-interactive"], &dir);

    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("No Brain Brew manifest found at brainbrew.yaml"));
    assert!(err.contains("Found possible manifests:"));
    assert!(err.contains("decks/sample/brainbrew.yaml"));
    assert!(err.contains("brainbrew translations --manifest decks/sample/brainbrew.yaml"));
}

#[test]
fn translations_help_does_not_advertise_rejected_static_editor_flags() {
    let output = run(["translations", "--help"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(!out.contains("--web-editor"));
    assert!(!out.contains("--apply-editor-edits"));
    assert!(!out.contains("translator editor"));
}

#[test]
fn translations_rejects_rejected_static_editor_flags() {
    let dir = temp_dir("translations-rejected-static-editor-flags");
    write_translation_workspace(&dir);

    let web = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--web-editor",
        "--out",
        dir.join("editor.html").to_str().unwrap(),
    ]);
    assert!(!web.status.success());
    assert!(stderr(&web).contains("unexpected translations argument \"--web-editor\""));

    let apply = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--apply-editor-edits",
        dir.join("edits.json").to_str().unwrap(),
    ]);
    assert!(!apply.status.success());
    assert!(stderr(&apply).contains("unexpected translations argument \"--apply-editor-edits\""));
}

#[test]
fn translations_reports_when_selected_target_has_no_dictionary_overlays() {
    let manifest = workspace_root().join("fixtures/ug-style/brainbrew.yaml");
    let output = run([
        "translations",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "full-demo",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("No translation dictionary coverage reports matched"));
    assert!(out.contains("translation.es"));
    assert!(out.contains("translation-sv.yaml"));
    assert!(out.contains("do not use a `translations:` dictionary"));
}

#[test]
fn translations_context_view_shows_missing_note_field_card_context() {
    let dir = temp_dir("translations-context-missing");
    write_translation_workspace(&dir);

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--context",
        "--source",
        "Sweden",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Translation context for target da-standard language da"));
    assert!(out.contains("missing_translation notes.note.sweden.fields.field.country"));
    assert!(out.contains("note: note.sweden"));
    assert!(out.contains("field: field.country (Country)"));
    assert!(out.contains("note fields (source/en | target/da):"));
    assert!(out.contains("* field.country (Country)"));
    assert!(out.contains("cards: template.country-capital [question]"));
    assert!(out.contains("source/en"));
    assert!(out.contains("target/da"));
    assert!(out.contains("Sweden"));
}

#[test]
fn translations_context_view_shows_ug_repeated_country_info_occurrences() {
    let manifest = workspace_root().join("fixtures/ultimate-geography/brainbrew.yaml");
    let output = run([
        "translations",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "es-standard",
        "--overlay",
        "overlay.translation.es",
        "--context",
        "--source",
        "Island of Indonesia.",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("duplicate source group: 3 occurrence(s)"));
    assert!(out.contains("note: note.bali"));
    assert!(out.contains("note: note.java"));
    assert!(out.contains("note: note.sumatra"));
    assert!(out.contains("field: field.country-info (Country info)"));
    assert!(out.contains("template.capital-country [answer]"));
    assert!(out.contains("template.country-capital [question+answer]"));
    assert!(out.contains("Island of Indonesia."));
    assert!(out.contains("Isla de Indonesia."));
}

#[test]
fn translations_context_view_wraps_long_ug_flag_similarity_context() {
    let manifest = workspace_root().join("fixtures/ultimate-geography/brainbrew.yaml");
    let output = run([
        "translations",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "es-standard",
        "--overlay",
        "overlay.translation.es",
        "--context",
        "--source",
        "blue background, red and white cross",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("note: note.faroe-islands"));
    assert!(out.contains("field: field.flag-similarity (Flag similarity)"));
    assert!(out.contains("template.flag-country [answer]"));
    assert!(out.contains("Iceland (blue background,"));
    assert!(out.contains("Islandia (fondo azul,"));
    assert!(out.contains("Noruega"));
}

#[test]
fn translations_context_view_json_exposes_reusable_context_model() {
    let manifest = workspace_root().join("fixtures/ultimate-geography/brainbrew.yaml");
    let output = run([
        "translations",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "es-standard",
        "--overlay",
        "overlay.translation.es",
        "--context",
        "--source",
        "Autonomous community of Spain.",
        "--json",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    let unit = &json["contexts"][0]["units"][0];
    assert_eq!(unit["status"], "direct_translation");
    assert_eq!(unit["note_id"], "note.canary-islands");
    assert_eq!(unit["field_id"], "field.country-info");
    assert_eq!(unit["field_name"], "Country info");
    assert_eq!(unit["note_fields"][0]["field_id"], "field.country");
    assert_eq!(unit["note_fields"][0]["source"], "Canary Islands");
    assert_eq!(unit["note_fields"][0]["translated"], "Canarias");
    assert_eq!(unit["source"], "Autonomous community of Spain.");
    assert_eq!(unit["translated"], "Comunidad autónoma de España.");
    let card_templates = unit["card_templates"].as_array().unwrap();
    assert!(card_templates.len() >= 4);
    let country_capital = card_templates
        .iter()
        .find(|card| card["template_id"] == "template.country-capital")
        .expect("country-capital card context is present");
    assert!(
        country_capital["question_format"]
            .as_str()
            .unwrap()
            .contains("{{Country info}}")
    );
    assert!(
        country_capital["answer_format"]
            .as_str()
            .unwrap()
            .contains("{{Capital}}")
    );
}

#[test]
fn translations_context_view_json_shows_structured_message_components() {
    let dir = temp_dir("translations-context-structured-message");
    write_structured_message_translation_workspace(&dir);

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "nb-standard",
        "--context",
        "--source",
        "blue background",
        "--json",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    let unit = &json["contexts"][0]["units"][0];
    assert_eq!(
        unit["path"],
        "notes.note.finland.fields.field.flag-similarity.message.2"
    );
    assert_eq!(unit["status"], "contextual_translation");
    assert_eq!(
        unit["message"]["source"],
        "Iceland (blue background with a white cross), Norway (red background with a blue cross)"
    );
    assert_eq!(
        unit["message"]["translated"],
        "Island (blå bakgrunn med hvitt kors), Norge (rød bakgrunn med blått kors)"
    );
    let components = unit["message"]["components"].as_array().unwrap();
    assert_eq!(components[0]["kind"], "field_ref");
    assert_eq!(
        components[0]["reference"],
        "notes.note.iceland.fields.field.country"
    );
    assert_eq!(components[0]["source"], "Iceland");
    assert_eq!(components[0]["translated"], "Island");
    assert_eq!(components[2]["kind"], "text");
    assert_eq!(
        components[2]["source"],
        "blue background with a white cross"
    );
    assert_eq!(components[2]["translated"], "blå bakgrunn med hvitt kors");
}

#[test]
fn translations_context_apply_uses_existing_translation_apply_path() {
    let dir = temp_dir("translations-context-apply");
    write_translation_workspace(&dir);
    let overlay_path = dir.join("da.yaml");

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--context",
        "--source",
        "Sweden",
        "--apply",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let updated = fs::read_to_string(overlay_path).unwrap();
    assert!(updated.contains("    Sweden: Sweden\n"));
}

#[test]
fn translations_context_view_can_filter_duplicate_missing_and_language() {
    let dir = temp_dir("translations-context-filters");
    write_translation_workspace(&dir);
    let deck_path = dir.join("deck.yaml");
    let deck = fs::read_to_string(&deck_path)
        .unwrap()
        .replace("field.country: Finland", "field.country: Sweden");
    fs::write(&deck_path, deck).unwrap();

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
        "--language",
        "da",
        "--context",
        "--duplicates",
        "--status",
        "missing",
        "--field",
        "field.country",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("missing_translation notes.note.sweden.fields.field.country"));
    assert!(out.contains("duplicate source group: 2 occurrence(s)"));
    assert!(!out.contains("notes.note.finland.fields.field.capital"));
}

#[test]
fn translations_summary_exports_compact_counts_by_language() {
    let dir = temp_dir("translations-summary");
    write_translation_workspace(&dir);

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
        "--summary",
        "--json",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert!(json.get("reports").is_none());
    let summaries = json["summaries"].as_array().unwrap();
    assert_eq!(summaries.len(), 1);
    let row = &summaries[0];
    assert_eq!(row["language"], "da");
    assert_eq!(
        row["targets"],
        serde_json::json!(["da-release", "da-standard"])
    );
    assert_eq!(row["overlay"], "overlay.translation.da");
    assert_eq!(row["file"], "da.yaml");
    assert_eq!(row["direct_translation"], 1);
    assert_eq!(row["contextual_translation"], 1);
    assert_eq!(row["no_change"], 0);
    assert_eq!(row["target_adaptation"], 1);
    assert_eq!(row["variable_translation"], 0);
    assert_eq!(row["adapter_id_translation"], 0);
    assert_eq!(row["untranslated_fallback"], 2);
    assert_eq!(row["missing_text_translation"], 2);
    assert_eq!(row["hidden_untranslated_fallback"], 0);
    assert_eq!(row["stale_invalid"], 1);
}

#[test]
fn translations_summary_human_output_uses_aligned_space_columns() {
    let dir = temp_dir("translations-summary-human");
    write_translation_workspace(&dir);

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
        "--summary",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(
        !out.contains('\t'),
        "summary should not rely on terminal tab stops:\n{out}"
    );
    assert!(
        out.contains("lang  tgt  direct"),
        "header should use compact padded columns:\n{out}"
    );
    assert!(
        !out.contains("overlay.translation.da"),
        "default summary should keep wide overlay/file columns out of the table:\n{out}"
    );
    for line in out.lines().skip(1) {
        assert!(
            line.chars().count() <= 90,
            "default summary line should stay compact, got {} chars:\n{line}\n\n{out}",
            line.chars().count()
        );
    }
    let full = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
        "--summary",
        "--full",
    ]);
    assert!(full.status.success(), "stderr: {}", stderr(&full));
    assert!(stdout(&full).contains("overlay.translation.da"));
    assert!(stdout(&full).contains("da.yaml"));
}

#[test]
fn translations_default_report_focuses_on_translatable_note_text() {
    let manifest = workspace_root().join("fixtures/ultimate-geography/brainbrew.yaml");
    let output = run([
        "translations",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "da-standard",
        "--overlay",
        "overlay.translation.da",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("missing text translations: 0"));
    assert!(out.contains("intentionally unchanged text: 329"));
    assert!(out.contains("hidden structural/media/tag fallbacks:"));
    assert!(out.contains("hint: use --full"));
    assert!(!out.contains("deck.description source="));
    assert!(!out.contains("notes.note.abkhazia.fields.field.flag"));
    assert!(!out.contains("notes.note.abkhazia.fields.field.capital"));
}

#[test]
fn ultimate_geography_language_overlays_have_no_actionable_missing_text() {
    let manifest = workspace_root().join("fixtures/ultimate-geography/brainbrew.yaml");
    let output = run([
        "translations",
        "--manifest",
        manifest.to_str().unwrap(),
        "--all-targets",
        "--json",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    let mut missing = Vec::new();
    for report in json["reports"].as_array().unwrap() {
        let file = report["file"].as_str().unwrap();
        if !file.starts_with("overlays/languages/") {
            continue;
        }
        let target = report["target"].as_str().unwrap();
        let overlay = report["overlay"].as_str().unwrap();
        for entry in report["entries"].as_array().unwrap() {
            if entry["category"] != "untranslated_fallback" {
                continue;
            }
            let path = entry["path"].as_str().unwrap();
            let source = entry["source"].as_str().unwrap();
            let actionable_note_field = path.starts_with("notes.")
                && path.contains(".fields.")
                && !path.ends_with(".fields.field.flag")
                && !path.ends_with(".fields.field.map")
                && !source.trim_start().starts_with("<img");
            if actionable_note_field {
                missing.push(format!("{target} {overlay} {path} source={source:?}"));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "language overlays should use translations or no_change for reviewed note text:\n{}",
        missing.join("\n")
    );
}

#[test]
fn translations_full_report_includes_structural_fallbacks() {
    let manifest = workspace_root().join("fixtures/ultimate-geography/brainbrew.yaml");
    let output = run([
        "translations",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "da-standard",
        "--overlay",
        "overlay.translation.da",
        "--full",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("deck.description source="));
    assert!(!out.contains("hidden structural/media/tag fallbacks:"));
}

#[test]
fn translations_interactive_language_options_come_from_translation_overlays_only() {
    let manifest = workspace_root().join("fixtures/ug-style/brainbrew.yaml");
    let output = run_with_stdin(
        [
            "translate",
            "--manifest",
            manifest.to_str().unwrap(),
            "--interactive",
        ],
        "\x1b[B\nq",
    );

    assert!(!output.status.success());
    let out = stdout(&output);
    assert!(out.contains("Language filter"));
    assert!(out.contains("es"));
    assert!(out.contains("sv"));
    assert!(!out.contains("capital"));
    assert!(!out.contains("hint"));
}

#[test]
fn translations_human_output_can_be_colored_but_json_stays_plain() {
    let dir = temp_dir("translations-color");
    write_translation_workspace(&dir);
    let manifest = dir.join("brainbrew.yaml");

    let colored = run_with_env(
        [
            "translations",
            "--manifest",
            manifest.to_str().unwrap(),
            "--target",
            "da-standard",
        ],
        &[("BRAINBREW_COLOR", "always")],
    );
    assert!(colored.status.success(), "stderr: {}", stderr(&colored));
    assert!(stdout(&colored).contains("\u{1b}["));

    let json = run_with_env(
        [
            "translations",
            "--manifest",
            manifest.to_str().unwrap(),
            "--target",
            "da-standard",
            "--json",
        ],
        &[("BRAINBREW_COLOR", "always")],
    );
    assert!(json.status.success(), "stderr: {}", stderr(&json));
    assert!(!stdout(&json).contains("\u{1b}["));
}

#[test]
fn translations_interactive_derives_selector_options_and_prints_equivalent_command() {
    let dir = temp_dir("translations-interactive-selectors");
    write_translation_workspace(&dir);

    let output = run_with_stdin(
        [
            "translate",
            "--manifest",
            dir.join("brainbrew.yaml").to_str().unwrap(),
            "--interactive",
        ],
        "\x1b[B\n\n\n\n\n",
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Brain Brew translation coverage"));
    assert!(out.contains("Target"));
    assert!(out.contains("da-release"));
    assert!(out.contains("da-standard"));
    assert!(!out.contains("Language filter"));
    assert!(!out.contains("Translation overlay"));
    assert!(out.contains("Scope"));
    assert!(out.contains("Equivalent command:"));
    assert!(out.contains("brainbrew translations"));
    assert!(out.contains("--target da-standard"));
    assert!(!out.contains("--language"));
    assert!(!out.contains("--overlay"));
    assert!(!out.contains("Select Target"));
    assert!(!out.contains("1. da-release"));
}

#[test]
fn translations_interactive_apply_can_mark_all_selected_rows_no_change() {
    let dir = temp_dir("translations-interactive-no-change-all");
    write_translation_workspace(&dir);
    let overlay_path = dir.join("da.yaml");

    let output = run_with_stdin(
        [
            "translations",
            "--manifest",
            dir.join("brainbrew.yaml").to_str().unwrap(),
            "--target",
            "da-standard",
            "--path-prefix",
            "notes.note.sweden.fields.field",
            "--apply",
            "--interactive",
        ],
        "\n\n\n",
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Action for selected missing translations"));
    assert!(out.contains("mark no-change for all 2 selected missing translations"));
    let updated = fs::read_to_string(overlay_path).unwrap();
    assert!(updated.contains("  no_change:\n"));
    assert!(updated.contains("    - Stockholm\n"));
    assert!(updated.contains("    - Sweden\n"));
}

#[test]
fn translations_interactive_apply_can_use_one_translation_stub_action_for_all_selected_rows() {
    let dir = temp_dir("translations-interactive-direct-all");
    write_translation_workspace(&dir);
    let overlay_path = dir.join("da.yaml");

    let output = run_with_stdin(
        [
            "translations",
            "--manifest",
            dir.join("brainbrew.yaml").to_str().unwrap(),
            "--target",
            "da-standard",
            "--path-prefix",
            "notes.note.sweden.fields.field",
            "--apply",
            "--interactive",
        ],
        "\n\x1b[B\n\n",
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains(
        "add direct source→source translation stubs for all 2 selected missing translations"
    ));
    let updated = fs::read_to_string(overlay_path).unwrap();
    assert!(updated.contains("    Stockholm: Stockholm\n"));
    assert!(updated.contains("    Sweden: Sweden\n"));
}

#[test]
fn translations_interactive_apply_can_insert_contextual_stub() {
    let dir = temp_dir("translations-interactive-contextual");
    write_translation_workspace(&dir);
    let overlay_path = dir.join("da.yaml");

    let output = run_with_stdin(
        [
            "translations",
            "--manifest",
            dir.join("brainbrew.yaml").to_str().unwrap(),
            "--target",
            "da-standard",
            "--path-prefix",
            "notes.note.sweden.fields.field.country",
            "--apply",
            "--interactive",
        ],
        "\n\x1b[B\x1b[B\n\n",
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let updated = fs::read_to_string(overlay_path).unwrap();
    assert!(updated.contains("    notes.note.sweden:\n      Sweden: Sweden\n"));
}

#[test]
fn translations_interactive_apply_can_insert_ignore_path() {
    let dir = temp_dir("translations-interactive-ignore");
    write_translation_workspace(&dir);
    let overlay_path = dir.join("da.yaml");

    let output = run_with_stdin(
        [
            "translations",
            "--manifest",
            dir.join("brainbrew.yaml").to_str().unwrap(),
            "--target",
            "da-standard",
            "--path-prefix",
            "notes.note.sweden.fields.field.country",
            "--apply",
            "--interactive",
        ],
        "\n\x1b[B\x1b[B\x1b[B\n\n",
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let updated = fs::read_to_string(overlay_path).unwrap();
    assert!(updated.contains("  ignore_paths:\n"));
    assert!(updated.contains("    - notes.note.sweden.fields.field.country\n"));
}

#[test]
fn translations_report_counts_no_change_as_intentionally_unchanged_text() {
    let dir = temp_dir("translations-no-change-report");
    write_translation_workspace(&dir);
    fs::write(
        dir.join("da.yaml"),
        r#"id: overlay.translation.da
kind: translation
translations:
  ignore_paths:
    - deck.*
    - note_types.*
    - notes.*.tags.*
    - notes.*.fields.field.flag
  no_change:
    - Stockholm
    - Sweden
"#,
    )
    .unwrap();

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--path-prefix",
        "notes.note.sweden.fields.field",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("intentionally unchanged text: 2"));
    assert!(out.contains("missing text translations: 0"));
    assert!(!out.contains("missing_translation notes.note.sweden.fields.field.country"));
}

#[test]
fn translations_reports_missing_stale_contextual_and_adaptations_without_modifying() {
    let dir = temp_dir("translations-report");
    write_translation_workspace(&dir);
    let overlay_path = dir.join("da.yaml");
    let before = fs::read_to_string(&overlay_path).unwrap();

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Translation coverage for target da-standard"));
    assert!(out.contains("contextual translations: 1"));
    assert!(out.contains("target adaptations: 1"));
    assert!(out.contains("stale_direct_key translations.direct.Removed source"));
    assert!(out.contains("missing_translation notes.note.sweden.fields.field.country"));
    assert_eq!(fs::read_to_string(overlay_path).unwrap(), before);
}

#[test]
fn translations_apply_inserts_sorted_direct_stubs_and_preserves_comments() {
    let dir = temp_dir("translations-apply");
    write_translation_workspace(&dir);
    let overlay_path = dir.join("da.yaml");

    let output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--path-prefix",
        "notes.note.sweden.fields.field.country",
        "--apply",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let updated = fs::read_to_string(overlay_path).unwrap();
    assert!(updated.contains("# translator note kept"));
    assert!(updated.contains("    Sweden: Sweden\n"));
}

#[test]
fn verify_translation_coverage_policy_can_be_lenient_or_strict() {
    let dir = temp_dir("translations-verify-policy");
    write_translation_workspace(&dir);
    let overlay_path = dir.join("da.yaml");
    fs::write(
        &overlay_path,
        r#"id: overlay.translation.da
kind: translation
translations:
  ignore_paths:
    - 'deck.*'
    - 'note_types.*'
    - 'notes.*.fields.field.flag'
    - 'notes.*.tags.*'
  direct:
    Finland: Finland
  contextual:
    notes.note:
      finland:
        Helsinki: Helsingfors
target_adaptations:
  notes.note.finland.fields.field.flag:
    expected_source: ''
    target: '<img src="fi-da.png">'
"#,
    )
    .unwrap();

    let lenient_output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--translation-coverage",
        "lenient",
    ]);
    assert!(
        lenient_output.status.success(),
        "stderr: {}",
        stderr(&lenient_output)
    );

    let strict_output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-release",
    ]);
    assert!(!strict_output.status.success());
    assert!(stderr(&strict_output).contains("translation coverage strict policy failed"));
    assert!(stderr(&strict_output).contains("notes.note.sweden.fields.field.country"));

    fs::write(
        &overlay_path,
        r#"id: overlay.translation.da
kind: translation
translations:
  ignore_paths:
    - 'deck.*'
    - 'note_types.*'
    - 'notes.*.fields.field.flag'
    - 'notes.*.tags.*'
  direct:
    Finland: Finland
  contextual:
    notes.note:
      finland:
        Helsinki: Helsingfors
  no_change:
    - Stockholm
    - Sweden
target_adaptations:
  notes.note.finland.fields.field.flag:
    expected_source: ''
    target: '<img src="fi-da.png">'
"#,
    )
    .unwrap();

    let strict_output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-release",
    ]);
    assert!(
        strict_output.status.success(),
        "stderr: {}",
        stderr(&strict_output)
    );
}

#[test]
fn stale_translations_warn_apply_and_fail_strict_verify() {
    let dir = temp_dir("stale-translations-verify");
    write_translation_workspace(&dir);
    let overlay_path = dir.join("da.yaml");
    fs::write(
        &overlay_path,
        r#"id: overlay.translation.da
kind: translation
translations:
  ignore_paths:
    - 'deck.*'
    - 'note_types.*'
    - 'notes.*.fields.field.flag'
    - 'notes.*.tags.*'
  direct:
    Finland: Finland
  contextual:
    notes.note:
      finland:
        Helsinki: Helsingfors
  no_change:
    - Sweden
target_adaptations:
  notes.note.finland.fields.field.flag:
    expected_source: ''
    target: '<img src="fi-da.png">'
stale_translations:
  - old_source: Old Stockholm
    new_source: Stockholm
    target: 'Stockholm på dansk'
"#,
    )
    .unwrap();

    let lenient_verify = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
    ]);
    assert!(
        lenient_verify.status.success(),
        "stderr: {}",
        stderr(&lenient_verify)
    );
    assert!(stderr(&lenient_verify).contains("stale translation warning"));
    assert!(stderr(&lenient_verify).contains("Old Stockholm"));

    let report_output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--status",
        "stale",
    ]);
    assert!(
        report_output.status.success(),
        "stderr: {}",
        stderr(&report_output)
    );
    assert!(stdout(&report_output).contains("stale_translation"));
    assert!(stdout(&report_output).contains("Old Stockholm"));
    assert!(!stdout(&report_output).contains("missing_translation"));

    let context_output = run([
        "translations",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--context",
        "--json",
        "--status",
        "stale_translation",
    ]);
    assert!(
        context_output.status.success(),
        "stderr: {}",
        stderr(&context_output)
    );
    let context_json: serde_json::Value =
        serde_json::from_slice(&context_output.stdout).expect("context output is JSON");
    assert_eq!(
        context_json["contexts"][0]["units"][0]["status"],
        "stale_translation"
    );
    assert_eq!(
        context_json["contexts"][0]["units"][0]["old_source"],
        "Old Stockholm"
    );

    let strict_verify = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-release",
    ]);
    assert!(!strict_verify.status.success());
    assert!(stderr(&strict_verify).contains("translation stale strict policy failed"));

    let compose_output = run([
        "compose",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
    ]);
    assert!(
        compose_output.status.success(),
        "stderr: {}",
        stderr(&compose_output)
    );
    assert!(stdout(&compose_output).contains("Stockholm på dansk"));
    assert!(stderr(&compose_output).contains("stale translation warning"));

    let raw_compose_output = run([
        "compose",
        dir.join("deck.yaml").to_str().unwrap(),
        "--overlay",
        overlay_path.to_str().unwrap(),
    ]);
    assert!(
        raw_compose_output.status.success(),
        "stderr: {}",
        stderr(&raw_compose_output)
    );
    assert!(stdout(&raw_compose_output).contains("Stockholm på dansk"));
    assert!(stderr(&raw_compose_output).contains("stale translation warning"));

    let export_dir = dir.join("crowdanki-out");
    let export_output = run([
        "export",
        "crowdanki",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "da-standard",
        "--out",
        export_dir.to_str().unwrap(),
    ]);
    assert!(
        export_output.status.success(),
        "stderr: {}",
        stderr(&export_output)
    );
    assert!(stderr(&export_output).contains("stale translation warning"));
    assert!(
        fs::read_to_string(export_dir.join("deck.json"))
            .unwrap()
            .contains("Stockholm på dansk")
    );
}

#[test]
fn export_crowdanki_copies_exact_declared_media_from_media_root_and_checks_hashes() {
    let dir = temp_dir("export-media");
    let deck_path = dir.join("deck.yaml");
    let media_root = dir.join("media");
    let export_dir = dir.join("crowdanki");
    fs::write(&deck_path, MEDIA_CANONICAL_YAML).unwrap();
    fs::create_dir_all(media_root.join("flags")).unwrap();
    fs::write(media_root.join("flags/fi.png"), b"flag-bytes").unwrap();
    fs::write(media_root.join("flags/undeclared.png"), b"extra").unwrap();

    let output = run([
        "export",
        "crowdanki",
        deck_path.to_str().unwrap(),
        "--media-root",
        media_root.to_str().unwrap(),
        "--out",
        export_dir.to_str().unwrap(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(
        fs::read(export_dir.join("media/flags/fi.png")).unwrap(),
        b"flag-bytes"
    );
    assert!(!export_dir.join("media/flags/undeclared.png").exists());
}

#[test]
fn verify_fails_on_referenced_but_undeclared_media_without_media_root() {
    let dir = temp_dir("verify-undeclared-media");
    fs::write(
        dir.join("deck.yaml"),
        MEDIA_CANONICAL_YAML.replace("path: flags/fi.png", "path: flags/other.png"),
    )
    .unwrap();
    fs::write(dir.join("brainbrew.yaml"), SIMPLE_MEDIA_MANIFEST_YAML).unwrap();

    let output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
    ]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("used but not declared"));
    assert!(stderr(&output).contains("flags/fi.png"));
}

#[test]
fn verify_checks_media_root_hashes() {
    let dir = temp_dir("verify-media");
    fs::write(dir.join("deck.yaml"), MEDIA_CANONICAL_YAML).unwrap();
    fs::write(dir.join("brainbrew.yaml"), SIMPLE_MEDIA_MANIFEST_YAML).unwrap();
    fs::create_dir_all(dir.join("media/flags")).unwrap();
    fs::write(dir.join("media/flags/fi.png"), b"wrong-bytes").unwrap();

    let output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
        "--media-root",
        dir.join("media").to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("media.flags-fi-png"));
    assert!(
        stderr(&output)
            .contains("expected 14873f4faae48052921f9272d948a369f775b2406e57a9b8d55fb94452b73948")
    );
    assert!(
        stderr(&output)
            .contains("actual 7c1d387f892b3c965dfc1951e2a92a2149cd103cef25c8ba5d0cc30a3a21063f")
    );
}

#[test]
fn verify_fails_on_empty_media_hashes_with_media_root() {
    let dir = temp_dir("verify-empty-media-hash");
    fs::write(
        dir.join("deck.yaml"),
        MEDIA_CANONICAL_YAML.replace(
            "14873f4faae48052921f9272d948a369f775b2406e57a9b8d55fb94452b73948",
            "''",
        ),
    )
    .unwrap();
    fs::write(dir.join("brainbrew.yaml"), SIMPLE_MEDIA_MANIFEST_YAML).unwrap();
    fs::create_dir_all(dir.join("media/flags")).unwrap();
    fs::write(dir.join("media/flags/fi.png"), b"flag-bytes").unwrap();

    let output = run([
        "verify",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
        "--media-root",
        dir.join("media").to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("empty sha256"));
    assert!(stderr(&output).contains("media.flags-fi-png"));
}

#[test]
fn media_hash_updates_source_hashes_and_preserves_includes() {
    let dir = temp_dir("media-hash");
    fs::create_dir_all(dir.join("content")).unwrap();
    fs::create_dir_all(dir.join("media/flags")).unwrap();
    fs::write(dir.join("content/description.md"), "Included description\n").unwrap();
    fs::write(dir.join("media/flags/fi.png"), b"flag-bytes").unwrap();
    fs::write(
        dir.join("deck.yaml"),
        MEDIA_CANONICAL_YAML
            .replace(
                "description: A deck with media.",
                "description: !include content/description.md",
            )
            .replace(
                "14873f4faae48052921f9272d948a369f775b2406e57a9b8d55fb94452b73948",
                "''",
            ),
    )
    .unwrap();
    fs::write(dir.join("brainbrew.yaml"), SIMPLE_MEDIA_MANIFEST_YAML).unwrap();

    let output = run([
        "media",
        "hash",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
        "--media-root",
        dir.join("media").to_str().unwrap(),
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let source = fs::read_to_string(dir.join("deck.yaml")).unwrap();
    assert!(source.contains("description: !include content/description.md"));
    assert!(
        source.contains("sha256: 14873f4faae48052921f9272d948a369f775b2406e57a9b8d55fb94452b73948")
    );
}

#[test]
fn explain_reports_expanded_stack_and_diff() {
    let dir = temp_dir("explain");
    write_manifest_workspace(&dir);

    let output = run([
        "explain",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("target: patched-via-dependency"));
    assert!(out.contains("1. patch.capital (capital.yaml)"));
    assert!(out.contains("modified notes.note.finland.fields.field.capital"));
}

#[test]
fn explain_reports_json_for_ui_consumers() {
    let dir = temp_dir("explain-json");
    write_manifest_workspace(&dir);
    fs::write(dir.join("brainbrew.yaml"), MANIFEST_WITH_PACKAGE_YAML).unwrap();

    let output = run([
        "explain",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "patched-via-dependency",
        "--json",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(json["package"]["id"], "anki-geo.ultimate-geography");
    assert_eq!(json["target"], "patched-via-dependency");
    assert_eq!(json["overlay_stack"][0]["id"], "patch.capital");
    assert_eq!(
        json["changes"][0]["path"],
        "notes.note.finland.fields.field.capital"
    );
}

#[test]
fn explain_reports_json_conflicts_for_ui_consumers() {
    let dir = temp_dir("explain-conflict-json");
    write_manifest_workspace(&dir);
    fs::write(dir.join("second.yaml"), SECOND_CAPITAL_OVERLAY_YAML).unwrap();
    fs::write(dir.join("brainbrew.yaml"), CONFLICT_MANIFEST_YAML).unwrap();

    let output = run([
        "explain",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "conflict",
        "--json",
    ]);

    assert!(!output.status.success());
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(json["target"], "conflict");
    assert_eq!(json["overlay_stack"][1]["id"], "patch.capital.second");
    assert_eq!(json["errors"][0]["kind"], "Conflict");
    assert_eq!(
        json["errors"][0]["path"],
        "notes.note.finland.fields.field.capital"
    );
}

#[test]
fn explain_reports_overlay_conflicts_with_stack_context() {
    let dir = temp_dir("explain-conflict");
    write_manifest_workspace(&dir);
    fs::write(dir.join("second.yaml"), SECOND_CAPITAL_OVERLAY_YAML).unwrap();
    fs::write(dir.join("brainbrew.yaml"), CONFLICT_MANIFEST_YAML).unwrap();

    let output = run([
        "explain",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "conflict",
    ]);

    assert!(!output.status.success());
    assert!(stdout(&output).contains("2. patch.capital.second (second.yaml)"));
    assert!(stderr(&output).contains("conflicts with earlier overlay"));
}

#[test]
fn diff_can_emit_note_field_changes_as_overlay() {
    let dir = temp_dir("diff-as-overlay");
    let left = dir.join("left.yaml");
    let right = dir.join("right.yaml");
    fs::write(&left, SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        &right,
        SAMPLE_CANONICAL_YAML.replace("field.capital: Helsinki", "field.capital: Helsingfors"),
    )
    .unwrap();

    let output = run([
        "diff",
        left.to_str().unwrap(),
        right.to_str().unwrap(),
        "--as-overlay",
        "--id",
        "overlay.patch.capital",
        "--kind",
        "patch",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("id: overlay.patch.capital"));
    assert!(out.contains("kind: patch"));
    assert!(out.contains("field.capital:"));
    assert!(out.contains("value: Helsingfors"));
    assert!(out.contains("expected_base:\n          value: Helsinki"));
}

#[test]
fn diff_as_overlay_emits_tag_and_adapter_id_changes() {
    let dir = temp_dir("diff-as-overlay-tags");
    let left = dir.join("left.yaml");
    let right = dir.join("right.yaml");
    fs::write(&left, SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        &right,
        SAMPLE_CANONICAL_YAML
            .replace("      - Nordic", "      - Baltic")
            .replace("ug-finland-guid", "ug-finland-guid-v2"),
    )
    .unwrap();

    let output = run([
        "diff",
        left.to_str().unwrap(),
        right.to_str().unwrap(),
        "--as-overlay",
        "--id",
        "overlay.patch.tags",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("      Baltic:\n        intent: add"));
    assert!(
        out.contains(
            "      Nordic:\n        intent: remove\n        expected_base: entity_present"
        )
    );
    assert!(out.contains("      crowdanki:guid:\n        intent: replace"));
    assert!(out.contains("        value: ug-finland-guid-v2"));
}

#[test]
fn diff_as_overlay_emits_media_reference_changes() {
    let dir = temp_dir("diff-as-overlay-media");
    let left = dir.join("left.yaml");
    let right = dir.join("right.yaml");
    fs::write(&left, SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        &right,
        SAMPLE_CANONICAL_YAML.replace(
            "tombstones: []",
            "  media.flags-se-png:\n    path: flags/se.png\n    sha256: ''\ntombstones: []",
        ),
    )
    .unwrap();

    let output = run([
        "diff",
        left.to_str().unwrap(),
        right.to_str().unwrap(),
        "--as-overlay",
        "--id",
        "overlay.patch.media",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("media:\n  media.flags-se-png:\n    intent: add"));
    assert!(out.contains("    path: flags/se.png"));
}

#[test]
fn diff_as_overlay_emits_note_additions_and_removals() {
    let dir = temp_dir("diff-as-overlay-notes");
    let left = dir.join("left.yaml");
    let added = dir.join("added.yaml");
    let removed = dir.join("removed.yaml");
    fs::write(&left, SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        &added,
        SAMPLE_CANONICAL_YAML.replace(
            "media:\n  media.flags-fi-png:",
            "  note.sweden:\n    note_type_id: note-type.country\n    fields:\n      field.capital: Stockholm\n      field.country: Sweden\n      field.flag: '<img src=\"se.png\">'\n    tags:\n      - Europe\n      - Nordic\n    adapter_ids:\n      crowdanki:guid: ug-sweden-guid\nmedia:\n  media.flags-fi-png:",
        ),
    )
    .unwrap();
    fs::write(&removed, SAMPLE_WITHOUT_NOTES_CANONICAL_YAML).unwrap();

    let add_output = run([
        "diff",
        left.to_str().unwrap(),
        added.to_str().unwrap(),
        "--as-overlay",
        "--id",
        "overlay.patch.note-add",
    ]);
    assert!(
        add_output.status.success(),
        "stderr: {}",
        stderr(&add_output)
    );
    assert!(stdout(&add_output).contains("  note.sweden:\n    intent: add"));
    assert!(stdout(&add_output).contains("    note:\n      note_type_id: note-type.country"));

    let remove_output = run([
        "diff",
        left.to_str().unwrap(),
        removed.to_str().unwrap(),
        "--as-overlay",
        "--id",
        "overlay.patch.note-remove",
    ]);
    assert!(
        remove_output.status.success(),
        "stderr: {}",
        stderr(&remove_output)
    );
    assert!(
        stdout(&remove_output)
            .contains("  note.finland:\n    intent: remove\n    expected_base: entity_present")
    );
}

#[test]
fn diff_reports_json_changes_by_stable_path() {
    let dir = temp_dir("diff-json");
    let left = dir.join("left.yaml");
    let right = dir.join("right.yaml");
    fs::write(&left, SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        &right,
        SAMPLE_CANONICAL_YAML.replace("field.capital: Helsinki", "field.capital: Helsingfors"),
    )
    .unwrap();

    let output = run([
        "diff",
        left.to_str().unwrap(),
        right.to_str().unwrap(),
        "--json",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("\"path\": \"notes.note.finland.fields.field.capital\""));
    assert!(stdout(&output).contains("\"after\": \"Helsingfors\""));
}

#[test]
fn diff_reports_human_readable_before_and_after_values() {
    let dir = temp_dir("diff-human");
    let left = dir.join("left.yaml");
    let right = dir.join("right.yaml");
    fs::write(&left, SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        &right,
        SAMPLE_CANONICAL_YAML
            .replace("field.capital: Helsinki", "field.capital: Helsingfors")
            .replace("field.country: Finland", "field.country: Suomi"),
    )
    .unwrap();

    let output = run(["diff", left.to_str().unwrap(), right.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("2 semantic changes"));
    assert!(out.contains("~ notes.note.finland.fields.field.capital"));
    assert!(out.contains("- Helsinki"));
    assert!(out.contains("+ Helsingfors"));
    assert!(out.contains("~ notes.note.finland.fields.field.country"));
    assert!(out.contains("- Finland"));
    assert!(out.contains("+ Suomi"));
}

#[test]
fn file_includes_work_across_validate_compose_export_verify_diff_and_fmt() {
    let dir = temp_dir("file-includes-workflows");
    write_include_workspace(&dir);
    let manifest = dir.join("brainbrew.yaml");

    let validate = run([
        "validate",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "localized",
    ]);
    assert!(validate.status.success(), "stderr: {}", stderr(&validate));

    let resolved = dir.join("resolved.yaml");
    let compose = run([
        "compose",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "localized",
        "--out",
        resolved.to_str().unwrap(),
    ]);
    assert!(compose.status.success(), "stderr: {}", stderr(&compose));
    let resolved_source = fs::read_to_string(&resolved).unwrap();
    assert!(resolved_source.contains("Overlay description from Markdown."));
    assert!(resolved_source.contains("<section class=\"front\">{{Country}}</section>"));
    assert!(resolved_source.contains("font-family: sans-serif"));
    assert!(!resolved_source.contains("!include"));

    let export_dir = dir.join("crowdanki");
    let export = run([
        "export",
        "crowdanki",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "localized",
        "--out",
        export_dir.to_str().unwrap(),
    ]);
    assert!(export.status.success(), "stderr: {}", stderr(&export));
    assert!(
        fs::read_to_string(export_dir.join("deck.json"))
            .unwrap()
            .contains("<section class=\\\"front\\\">{{Country}}</section>")
    );

    let verify = run([
        "verify",
        "--manifest",
        manifest.to_str().unwrap(),
        "--all-targets",
    ]);
    assert!(verify.status.success(), "stderr: {}", stderr(&verify));

    let base_resolved = dir.join("base-resolved.yaml");
    let compose_base = run([
        "compose",
        "--manifest",
        manifest.to_str().unwrap(),
        "--target",
        "base",
        "--out",
        base_resolved.to_str().unwrap(),
    ]);
    assert!(
        compose_base.status.success(),
        "stderr: {}",
        stderr(&compose_base)
    );
    let diff = run([
        "diff",
        dir.join("deck.yaml").to_str().unwrap(),
        base_resolved.to_str().unwrap(),
    ]);
    assert!(diff.status.success(), "stderr: {}", stderr(&diff));
    assert!(stdout(&diff).contains("no semantic changes"));

    let deck_for_fmt = dir.join("deck-to-format.yaml");
    fs::copy(dir.join("deck.yaml"), &deck_for_fmt).unwrap();
    let original = fs::read_to_string(&deck_for_fmt).unwrap();
    let fmt = run(["fmt", deck_for_fmt.to_str().unwrap()]);
    assert!(fmt.status.success(), "stderr: {}", stderr(&fmt));
    let formatted = fs::read_to_string(deck_for_fmt).unwrap();
    assert!(formatted.contains("description: !include ./content/../content/base-description.md"));
    assert!(formatted.contains("question_format: !include templates/front.html"));
    assert!(formatted.contains("answer_format: !include templates/back.html"));
    assert!(formatted.contains("styling: !include styles/card.css"));
    assert!(!formatted.contains("Base deck description."));
    assert!(!formatted.contains("<section class=\"front\">{{Country}}</section>"));
    assert_eq!(
        include_directives(&formatted),
        include_directives(&original)
    );
}

#[test]
fn fmt_preserves_include_markers_and_does_not_inline_included_content() {
    let dir = temp_dir("fmt-preserves-includes");
    write_include_workspace(&dir);
    let deck_path = dir.join("deck.yaml");
    let original = fs::read_to_string(&deck_path).unwrap();

    let output = run(["fmt", deck_path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let formatted = fs::read_to_string(deck_path).unwrap();
    assert_eq!(
        include_directives(&formatted),
        include_directives(&original)
    );
    assert!(formatted.contains("description: !include ./content/../content/base-description.md"));
    assert!(formatted.contains("question_format: !include templates/front.html"));
    assert!(formatted.contains("answer_format: !include templates/back.html"));
    assert!(formatted.contains("styling: !include styles/card.css"));
    assert!(!formatted.contains("Base deck description."));
    assert!(!formatted.contains("<section class=\"front\">{{Country}}</section>"));
}

#[test]
fn fmt_on_include_bearing_file_is_idempotent() {
    let dir = temp_dir("fmt-include-idempotent");
    write_include_workspace(&dir);
    let deck_path = dir.join("deck.yaml");

    let first = run(["fmt", deck_path.to_str().unwrap()]);
    assert!(first.status.success(), "stderr: {}", stderr(&first));
    let after_first = fs::read_to_string(&deck_path).unwrap();

    let second = run(["fmt", deck_path.to_str().unwrap()]);
    assert!(second.status.success(), "stderr: {}", stderr(&second));
    let after_second = fs::read_to_string(deck_path).unwrap();

    assert_eq!(after_second, after_first);
    assert!(
        after_second.contains("description: !include ./content/../content/base-description.md")
    );
    assert!(!after_second.contains("Base deck description."));
}

#[test]
fn fmt_normalizes_non_include_parts_of_include_bearing_file() {
    let dir = temp_dir("fmt-include-normalizes-siblings");
    write_include_workspace(&dir);
    let deck_path = dir.join("deck.yaml");
    let source = fs::read_to_string(&deck_path).unwrap();
    fs::write(
        &deck_path,
        source.replace("name: Include Fixture", "name: 'Include Fixture'"),
    )
    .unwrap();

    let output = run(["fmt", deck_path.to_str().unwrap()]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let formatted = fs::read_to_string(deck_path).unwrap();
    assert!(formatted.contains("name: Include Fixture"));
    assert!(!formatted.contains("name: 'Include Fixture'"));
    assert!(formatted.contains("description: !include ./content/../content/base-description.md"));
    assert!(!formatted.contains("Base deck description."));
}

#[test]
fn verify_rejects_noncanonical_include_bearing_file_and_accepts_canonical_one() {
    let canonical = temp_dir("verify-include-canonical");
    write_include_workspace(&canonical);
    let accepted = run([
        "verify",
        "--manifest",
        canonical.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
    ]);
    assert!(accepted.status.success(), "stderr: {}", stderr(&accepted));

    let noncanonical = temp_dir("verify-include-noncanonical");
    write_include_workspace(&noncanonical);
    let deck_path = noncanonical.join("deck.yaml");
    let source = fs::read_to_string(&deck_path).unwrap();
    fs::write(
        &deck_path,
        source.replace("name: Include Fixture", "name: 'Include Fixture'"),
    )
    .unwrap();

    let rejected = run([
        "verify",
        "--manifest",
        noncanonical.join("brainbrew.yaml").to_str().unwrap(),
        "--all-targets",
    ]);

    assert!(!rejected.status.success());
    assert!(
        stderr(&rejected).contains("deck.yaml is not in canonical format"),
        "stderr: {}",
        stderr(&rejected)
    );
}

#[test]
fn file_include_errors_name_referring_yaml_path_and_included_path() {
    let dir = temp_dir("file-include-errors");
    write_include_workspace(&dir);
    let deck_path = dir.join("deck.yaml");
    let deck = fs::read_to_string(&deck_path).unwrap();
    fs::write(
        &deck_path,
        deck.replace(
            "description: !include ./content/../content/base-description.md",
            "description: !include content/missing.md",
        ),
    )
    .unwrap();

    let output = run([
        "validate",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "base",
    ]);

    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("deck.description"), "stderr: {err}");
    assert!(err.contains("content/missing.md"), "stderr: {err}");
}

#[test]
fn file_includes_reject_package_root_escape_unless_safe_root_is_configured() {
    let root = temp_dir("file-include-roots");
    let package = root.join("package");
    let shared = root.join("shared");
    fs::create_dir_all(&package).unwrap();
    fs::create_dir_all(&shared).unwrap();
    write_include_workspace(&package);
    fs::write(
        shared.join("description.md"),
        "Shared safe-root description.\n",
    )
    .unwrap();
    let deck_path = package.join("deck.yaml");
    let deck = fs::read_to_string(&deck_path).unwrap();
    fs::write(
        &deck_path,
        deck.replace(
            "description: !include ./content/../content/base-description.md",
            "description: !include ../shared/description.md",
        ),
    )
    .unwrap();

    let rejected = run([
        "validate",
        "--manifest",
        package.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "base",
    ]);
    assert!(!rejected.status.success());
    let err = stderr(&rejected);
    assert!(err.contains("deck.description"), "stderr: {err}");
    assert!(err.contains("../shared/description.md"), "stderr: {err}");
    assert!(err.contains("escapes package root"), "stderr: {err}");

    let manifest_path = package.join("brainbrew.yaml");
    fs::write(
        &manifest_path,
        fs::read_to_string(&manifest_path).unwrap().replace(
            "base: deck.yaml\n",
            "base: deck.yaml\ninclude_roots:\n  - ../shared\n",
        ),
    )
    .unwrap();
    let accepted = run([
        "validate",
        "--manifest",
        manifest_path.to_str().unwrap(),
        "--target",
        "base",
    ]);
    assert!(accepted.status.success(), "stderr: {}", stderr(&accepted));
}

#[test]
fn file_include_cycles_are_reported_with_yaml_path_and_include_chain() {
    let dir = temp_dir("file-include-cycle");
    write_include_workspace(&dir);
    fs::write(dir.join("content/a.md"), "!include content/b.md\n").unwrap();
    fs::write(dir.join("content/b.md"), "!include content/a.md\n").unwrap();
    let deck_path = dir.join("deck.yaml");
    let deck = fs::read_to_string(&deck_path).unwrap();
    fs::write(
        &deck_path,
        deck.replace(
            "description: !include ./content/../content/base-description.md",
            "description: !include content/a.md",
        ),
    )
    .unwrap();

    let output = run([
        "validate",
        "--manifest",
        dir.join("brainbrew.yaml").to_str().unwrap(),
        "--target",
        "base",
    ]);

    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("deck.description"), "stderr: {err}");
    assert!(err.contains("content/a.md"), "stderr: {err}");
    assert!(err.contains("cyclic"), "stderr: {err}");
}

#[test]
fn file_includes_are_rejected_outside_scalar_content_fields() {
    let dir = temp_dir("file-include-invalid-target");
    fs::create_dir_all(dir.join("content")).unwrap();
    fs::write(dir.join("content/not-notes.yaml"), "{}\n").unwrap();
    let deck_path = dir.join("invalid.yaml");
    fs::write(
        &deck_path,
        r#"deck:
  id: deck.invalid-include
  name: Invalid Include
  description: Valid scalar description.
note_types: {}
notes: !include content/not-notes.yaml
media: {}
tombstones: []
"#,
    )
    .unwrap();

    let output = run(["validate", deck_path.to_str().unwrap()]);

    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("notes"), "stderr: {err}");
    assert!(err.contains("content/not-notes.yaml"), "stderr: {err}");
    assert!(err.contains("scalar content"), "stderr: {err}");
}

#[test]
fn export_and_import_crowdanki_deck_folder() {
    let dir = temp_dir("crowdanki-roundtrip");
    let deck_path = dir.join("deck.yaml");
    let export_dir = dir.join("crowdanki");
    let imported_path = dir.join("imported.yaml");
    fs::write(&deck_path, SAMPLE_CANONICAL_YAML).unwrap();

    let export_output = run([
        "export",
        "crowdanki",
        deck_path.to_str().unwrap(),
        "--out",
        export_dir.to_str().unwrap(),
    ]);
    assert!(
        export_output.status.success(),
        "stderr: {}",
        stderr(&export_output)
    );
    assert!(
        fs::read_to_string(export_dir.join("deck.json"))
            .unwrap()
            .contains("ug-finland-guid")
    );

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
    assert!(
        fs::read_to_string(imported_path)
            .unwrap()
            .contains("id: deck.ultimate-geography")
    );
}

fn run<const N: usize>(args: [&str; N]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .output()
        .expect("command runs")
}

fn run_in_dir<const N: usize>(args: [&str; N], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("command runs")
}

fn run_with_env<const N: usize>(args: [&str; N], envs: &[(&str, &str)]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_brainbrew"));
    command.args(args);
    for (name, value) in envs {
        command.env(name, value);
    }
    command.output().expect("command runs")
}

fn run_with_stdin<const N: usize>(args: [&str; N], stdin: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("command spawns");
    child
        .stdin
        .as_mut()
        .expect("stdin is piped")
        .write_all(stdin.as_bytes())
        .expect("stdin writes");
    child.wait_with_output().expect("command runs")
}

fn run_with_cache<const N: usize>(args: [&str; N], cache: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .env("BRAINBREW_CACHE_DIR", cache)
        .output()
        .expect("command runs")
}

struct RunningWorkbenchServer {
    child: Child,
    base_url: String,
    _stdout: BufReader<ChildStdout>,
}

impl RunningWorkbenchServer {
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

impl Drop for RunningWorkbenchServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn_workbench_server<const N: usize>(args: [&str; N]) -> RunningWorkbenchServer {
    spawn_workbench_server_with_env(args, &[])
}

fn spawn_workbench_server_with_env<const N: usize>(
    args: [&str; N],
    envs: &[(&str, &str)],
) -> RunningWorkbenchServer {
    let mut command = Command::new(env!("CARGO_BIN_EXE_brainbrew"));
    command.args(args);
    for (name, value) in envs {
        command.env(name, value);
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("workbench server spawns");
    let stdout = child.stdout.take().expect("server stdout is piped");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).expect("server prints URL");
    let Some(base_url) = line.strip_prefix("Workbench listening at ") else {
        panic!("unexpected workbench server line: {line:?}");
    };
    RunningWorkbenchServer {
        child,
        base_url: base_url.trim().to_owned(),
        _stdout: reader,
    }
}

fn get_json(url: &str) -> serde_json::Value {
    let response = ureq::get(url).call().expect("GET succeeds");
    assert_eq!(response.status(), 200);
    serde_json::from_str(&response.into_string().unwrap()).expect("response is JSON")
}

fn assert_get_status_contains(url: &str, expected_status: u16, expected_body: &str) {
    let error = ureq::get(url)
        .call()
        .expect_err("GET should fail with expected status");
    match error {
        ureq::Error::Status(status, response) => {
            assert_eq!(status, expected_status);
            assert!(
                response.into_string().unwrap().contains(expected_body),
                "error body should contain {expected_body:?}"
            );
        }
        other => panic!("unexpected GET error: {other}"),
    }
}

fn collect_api_media_paths(value: &serde_json::Value, paths: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::String(text) => collect_api_media_paths_from_text(text, paths),
        serde_json::Value::Array(items) => {
            for item in items {
                collect_api_media_paths(item, paths);
            }
        }
        serde_json::Value::Object(map) => {
            for item in map.values() {
                collect_api_media_paths(item, paths);
            }
        }
        _ => {}
    }
}

fn collect_api_media_paths_from_text(text: &str, paths: &mut BTreeSet<String>) {
    let marker = "/api/media/";
    let mut rest = text;
    while let Some(start) = rest.find(marker) {
        let after_marker = &rest[start + marker.len()..];
        let path_end = after_marker
            .find(|character: char| {
                matches!(
                    character,
                    '"' | '\'' | '<' | '>' | ')' | '(' | ' ' | '\n' | '\r' | '\t'
                )
            })
            .unwrap_or(after_marker.len());
        let path = &after_marker[..path_end];
        if !path.is_empty() {
            paths.insert(path.to_owned());
        }
        rest = &after_marker[path_end..];
    }
}

fn assert_media_response_serves_declared_bytes(
    server: &RunningWorkbenchServer,
    media_root: &Path,
    media_path: &str,
) {
    let url = server.url(&format!("/api/media/{media_path}"));
    let candidate = media_root.join(media_path);
    let response = match ureq::get(&url).call() {
        Ok(response) => response,
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().unwrap_or_default();
            panic!(
                "media path {media_path:?} served from candidate {} via {url} returned HTTP {status}: {body}",
                candidate.display()
            );
        }
        Err(error) => panic!(
            "media path {media_path:?} served from candidate {} via {url} failed: {error}",
            candidate.display()
        ),
    };
    assert_eq!(
        response.status(),
        200,
        "media path {media_path:?} served from candidate {} via {url} returned unexpected status",
        candidate.display()
    );
    assert_eq!(
        response.header("content-type").unwrap_or_default(),
        expected_media_content_type(media_path),
        "media path {media_path:?} served from candidate {} via {url} returned wrong content type",
        candidate.display()
    );
    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .expect("read media response body");
    assert!(
        !bytes.is_empty(),
        "media path {media_path:?} served from candidate {} via {url} returned an empty body",
        candidate.display()
    );
    let body = String::from_utf8_lossy(&bytes);
    assert!(
        !body.contains("Missing media asset"),
        "media path {media_path:?} served from candidate {} via {url} returned the missing-media placeholder: {body}",
        candidate.display()
    );
}

fn expected_media_content_type(media_path: &str) -> &'static str {
    if media_path.ends_with(".svg") {
        "image/svg+xml"
    } else if media_path.ends_with(".png") {
        "image/png"
    } else {
        "application/octet-stream"
    }
}

fn post_json(url: &str, body: serde_json::Value) -> serde_json::Value {
    let response = ureq::post(url)
        .set("content-type", "application/json")
        .send_string(&body.to_string())
        .expect("POST succeeds");
    assert_eq!(response.status(), 200);
    serde_json::from_str(&response.into_string().unwrap()).expect("response is JSON")
}

fn post_json_error(url: &str, body: serde_json::Value) -> (u16, String) {
    let error = ureq::post(url)
        .set("content-type", "application/json")
        .send_string(&body.to_string())
        .expect_err("POST should fail");
    match error {
        ureq::Error::Status(status, response) => (status, response.into_string().unwrap()),
        other => panic!("unexpected POST error: {other}"),
    }
}

fn atomic_translation_apply_request(value_suffix: &str) -> serde_json::Value {
    serde_json::json!({
        "language": "da",
        "target": "standard",
        "overlay": "base",
        "edits": [{
            "kind": "translation",
            "path": "notes.note.finland.fields.field.capital",
            "source": "Helsinki",
            "value": format!("Helsingfors {value_suffix}"),
            "mode": "direct"
        }]
    })
}

fn atomic_multi_file_apply_request(value_suffix: &str) -> serde_json::Value {
    serde_json::json!({
        "language": "da",
        "target": "standard",
        "overlay": "base",
        "edits": [
            {
                "kind": "source",
                "path": "notes.note.finland.fields.field.country",
                "source": "Finland",
                "value": format!("Finland {value_suffix}"),
                "scope": "field",
                "impact_action": "stale_translation"
            },
            {
                "kind": "translation",
                "language": "da",
                "target": "standard",
                "overlay": "base",
                "path": "notes.note.finland.fields.field.capital",
                "source": "Helsinki",
                "value": format!("Helsingfors {value_suffix}"),
                "mode": "direct"
            },
            {
                "kind": "translation",
                "language": "nb",
                "target": "standard",
                "overlay": "base",
                "path": "notes.note.finland.fields.field.capital",
                "source": "Helsinki",
                "value": format!("Helsinki {value_suffix}"),
                "mode": "direct"
            }
        ]
    })
}

fn quoted_path_containing(html: &str, needle: &str) -> String {
    let needle_position = html
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} appears in HTML"));
    let path_start = html[..needle_position]
        .rfind('"')
        .expect("asset path starts with a quote")
        + 1;
    let path_end = needle_position
        + html[needle_position..]
            .find('"')
            .expect("asset path ends with a quote");
    html[path_start..path_end].to_owned()
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under crates/brain-brew-cli")
        .to_path_buf()
}

fn write_manifest_workspace(dir: &Path) {
    fs::write(dir.join("deck.yaml"), SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(dir.join("capital.yaml"), CAPITAL_OVERLAY_YAML).unwrap();
    fs::write(dir.join("noop.yaml"), NOOP_OVERLAY_YAML).unwrap();
    fs::write(dir.join("brainbrew.yaml"), MANIFEST_YAML).unwrap();
}

fn write_workbench_workspace(dir: &Path) {
    fs::write(dir.join("deck.yaml"), SAMPLE_CANONICAL_YAML).unwrap();
    write_workbench_manifest_and_overlay(
        dir,
        r#"id: overlay.translation.da
kind: translation
translations:
  direct:
    Finland: Finland
"#,
    );
}

fn write_ug_media_diagnostics_root(media_root: &Path) {
    fs::write(
        media_root.join("ug-flag-abkhazia.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="16"><rect width="24" height="16" fill="#2563eb"/></svg>"##,
    )
    .unwrap();
    fs::write(
        media_root.join("ug-map-abkhazia.png"),
        b"deterministic png bytes for UG map media diagnostics",
    )
    .unwrap();
}

fn write_workbench_repeated_source_workspace(dir: &Path) {
    write_workbench_repeated_source_deck(dir);
    write_workbench_manifest_and_overlay(
        dir,
        r#"id: overlay.translation.da
kind: translation
translations:
  direct:
    Estonia: Estland
    Finland: Finland
    Shared capital: Fælles hovedstad
"#,
    );
}

fn write_workbench_repeated_source_contextual_workspace(dir: &Path) {
    write_workbench_repeated_source_deck(dir);
    write_workbench_manifest_and_overlay(
        dir,
        r#"id: overlay.translation.da
kind: translation
translations:
  direct:
    Estonia: Estland
    Finland: Finland
  contextual:
    notes.note.estonia:
      Shared capital: Estisk fælles
    notes.note.finland:
      Shared capital: Finsk fælles
"#,
    );
}

fn write_workbench_optional_metadata_workspace(dir: &Path) {
    fs::write(dir.join("deck.yaml"), SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        dir.join("da.yaml"),
        r#"id: overlay.translation.da
kind: translation
translations:
  direct:
    Finland: Finland
    Helsinki: Helsinki
  stale_translations:
    - old_source: Old Workbench
      new_source: Ultimate Geography
      target: Gammel arbejdsbord
      context: deck.name
  adapter_ids:
    crowdanki:uuid:
      43c5ba66-9a65-11e8-90c9-a0481cc15658: da-deck-uuid
    crowdanki:guid:
      ug-finland-guid: da-finland-guid
"#,
    )
    .unwrap();
    fs::write(
        dir.join("brainbrew.yaml"),
        r#"base: deck.yaml
overlays:
  overlay.translation.da:
    file: da.yaml
    kind: translation
targets:
  da-standard:
    overlays:
      - overlay.translation.da
  en-standard:
    overlays: []
languages:
  da:
    display_name: Danish
    translation_overlays:
      base: overlay.translation.da
    primary_target: standard
    targets:
      standard: da-standard
  en:
    display_name: English
    source: true
    primary_target: standard
    targets:
      standard: en-standard
translation_profile:
  structural_fields:
    - field.flag
  metadata_categories:
    - key: deck-metadata
      label: Deck metadata
      paths:
        - deck.name
        - deck.description
    - key: note-type-name
      label: Note type names
      paths:
        - note_types.*.name
    - key: field-label
      label: Field labels
      paths:
        - note_types.*.fields.*.name
    - key: card-template-name
      label: Card template names
      paths:
        - note_types.*.card_templates.*.name
    - key: tag
      label: Tags
      paths:
        - notes.*.tags.*
  metadata_paths:
    - deck.*
    - note_types.*.fields.*.name
    - note_types.*.card_templates.*.name
    - notes.*.tags.*
  metadata_exclude_paths:
    - deck.adapter_ids.*
    - note_types.*.adapter_ids.*
    - note_types.*.card_templates.*.adapter_ids.*
    - notes.*.adapter_ids.*
  metadata_category_order:
    - deck-metadata
    - note-type-name
    - field-label
    - card-template-name
    - tag
"#,
    )
    .unwrap();
}

fn write_workbench_repeated_source_multi_language_workspace(dir: &Path) {
    write_workbench_repeated_source_deck(dir);
    fs::write(
        dir.join("da.yaml"),
        r#"id: overlay.translation.da
kind: translation
translations:
  direct:
    Estonia: Estland
    Finland: Finland
    Shared capital: Fælles hovedstad
"#,
    )
    .unwrap();
    fs::write(
        dir.join("nb.yaml"),
        r#"id: overlay.translation.nb
kind: translation
translations:
  direct:
    Estonia: Estland
    Finland: Finland
    Shared capital: Felles hovedstad
"#,
    )
    .unwrap();
    fs::write(
        dir.join("brainbrew.yaml"),
        r#"base: deck.yaml
overlays:
  overlay.translation.da:
    file: da.yaml
    kind: translation
  overlay.translation.nb:
    file: nb.yaml
    kind: translation
targets:
  da-standard:
    overlays:
      - overlay.translation.da
  en-standard:
    overlays: []
  nb-standard:
    overlays:
      - overlay.translation.nb
languages:
  da:
    display_name: Danish
    translation_overlays:
      base: overlay.translation.da
    primary_target: standard
    targets:
      standard: da-standard
  en:
    display_name: English
    source: true
    primary_target: standard
    targets:
      standard: en-standard
  nb:
    display_name: Norwegian
    translation_overlays:
      base: overlay.translation.nb
    primary_target: standard
    targets:
      standard: nb-standard
translation_profile:
  structural_fields:
    - field.flag
"#,
    )
    .unwrap();
}

fn write_workbench_repeated_source_deck(dir: &Path) {
    fs::write(
        dir.join("deck.yaml"),
        r#"deck:
  id: deck.workbench-repeated-source
  name: Workbench Repeated Source
  description: A small source edit fixture.
  adapter_ids:
    crowdanki:uuid: repeated-source-deck-uuid
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.country
      - field.capital
      - field.flag
    fields:
      field.capital:
        name: Capital
      field.country:
        name: Country
      field.flag:
        name: Flag
    card_template_order:
      - template.country-capital
    card_templates:
      template.country-capital:
        name: Country - Capital
        question_format: '{{Country}}'
        answer_format: '{{FrontSide}}<hr id=answer>{{Capital}}'
        adapter_ids: {}
    styling: ''
    adapter_ids:
      crowdanki:uuid: repeated-source-note-type-uuid
notes:
  note.finland:
    note_type_id: note-type.country
    fields:
      field.capital: Shared capital
      field.country: Finland
      field.flag: '<img src="flags/fi.png">'
    tags:
      - Europe
    adapter_ids:
      crowdanki:guid: repeated-source-fi-guid
  note.estonia:
    note_type_id: note-type.country
    fields:
      field.capital: Shared capital
      field.country: Estonia
      field.flag: '<img src="ee.png">'
    tags:
      - Europe
    adapter_ids:
      crowdanki:guid: repeated-source-ee-guid
media: {}
tombstones: []
"#,
    )
    .unwrap();
}

fn write_multi_language_workbench_workspace(dir: &Path) {
    fs::write(dir.join("deck.yaml"), SAMPLE_CANONICAL_YAML).unwrap();
    fs::write(
        dir.join("da.yaml"),
        r#"id: overlay.translation.da
kind: translation
translations:
  direct:
    Finland: Finland
"#,
    )
    .unwrap();
    fs::write(
        dir.join("nb.yaml"),
        r#"id: overlay.translation.nb
kind: translation
translations: {}
"#,
    )
    .unwrap();
    fs::write(
        dir.join("brainbrew.yaml"),
        r#"base: deck.yaml
overlays:
  overlay.translation.da:
    file: da.yaml
    kind: translation
  overlay.translation.nb:
    file: nb.yaml
    kind: translation
targets:
  da-standard:
    overlays:
      - overlay.translation.da
  en-standard:
    overlays: []
  nb-standard:
    overlays:
      - overlay.translation.nb
languages:
  da:
    display_name: Danish
    translation_overlays:
      base: overlay.translation.da
    primary_target: standard
    targets:
      standard: da-standard
  en:
    display_name: English
    source: true
    primary_target: standard
    targets:
      standard: en-standard
  nb:
    display_name: Norwegian
    translation_overlays:
      base: overlay.translation.nb
    primary_target: standard
    targets:
      standard: nb-standard
translation_profile:
  structural_fields:
    - field.flag
  metadata_categories:
    - key: deck-metadata
      label: Deck metadata
      paths:
        - deck.name
        - deck.description
  metadata_paths:
    - deck.*
"#,
    )
    .unwrap();
}

fn write_workbench_manifest_and_overlay(dir: &Path, overlay: &str) {
    fs::write(dir.join("da.yaml"), overlay).unwrap();
    fs::write(
        dir.join("brainbrew.yaml"),
        r#"base: deck.yaml
overlays:
  overlay.translation.da:
    file: da.yaml
    kind: translation
targets:
  da-standard:
    overlays:
      - overlay.translation.da
  en-standard:
    overlays: []
languages:
  da:
    display_name: Danish
    translation_overlays:
      base: overlay.translation.da
    primary_target: standard
    targets:
      standard: da-standard
  en:
    display_name: English
    source: true
    primary_target: standard
    targets:
      standard: en-standard
translation_profile:
  structural_fields:
    - field.flag
  metadata_categories:
    - key: deck-metadata
      label: Deck metadata
      paths:
        - deck.name
        - deck.description
  metadata_paths:
    - deck.*
"#,
    )
    .unwrap();
}

fn write_translation_workspace(dir: &Path) {
    let deck = SAMPLE_CANONICAL_YAML
        .replace("field.flag: '<img src=\"flags/fi.png\">'", "field.flag: ''")
        .replace(
            "media:\n",
            r#"  note.sweden:
    note_type_id: note-type.country
    fields:
      field.capital: Stockholm
      field.country: Sweden
      field.flag: '<img src="se.png">'
    tags:
      - Europe
      - Nordic
    adapter_ids:
      crowdanki:guid: ug-sweden-guid
media:
"#,
        )
        .replace(
            "media:\n  media.flags-fi-png:",
            "media:\n  media.flags-fi-da-png:\n    path: fi-da.png\n    sha256: ''\n  media.flags-fi-png:",
        )
        .replace(
            "media.flags-fi-png:\n    path: flags/fi.png\n    sha256: ''",
            "media.flags-fi-png:\n    path: flags/fi.png\n    sha256: ''\n  media.flags-se-png:\n    path: se.png\n    sha256: ''",
        );
    fs::write(dir.join("deck.yaml"), deck).unwrap();
    fs::write(
        dir.join("da.yaml"),
        r#"id: overlay.translation.da
kind: translation
# translator note kept
translations:
  ignore_paths:
    - deck.*
    - note_types.*
    - notes.*.tags.*
    - notes.*.fields.field.flag
  direct:
    Finland: Finland
    Removed source: Fjernet
  contextual:
    notes.note.finland:
      Helsinki: Helsingfors
target_adaptations:
  notes.note.finland.fields.field.flag:
    expected_source: ''
    target: '<img src="fi-da.png">'
"#,
    )
    .unwrap();
    fs::write(
        dir.join("brainbrew.yaml"),
        r#"base: deck.yaml
overlays:
  overlay.translation.da:
    file: da.yaml
    kind: translation
targets:
  da-release:
    overlays:
      - overlay.translation.da
    translation_coverage: strict
  da-standard:
    overlays:
      - overlay.translation.da
"#,
    )
    .unwrap();
}

fn write_structured_message_translation_workspace(dir: &Path) {
    fs::write(
        dir.join("deck.yaml"),
        r#"deck:
  id: deck.structured-message
  name: Structured Message Fixture
  description: Structured message translation fixture.
  adapter_ids: {}
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.country
      - field.capital
      - field.flag
      - field.flag-similarity
    fields:
      field.country:
        name: Country
      field.capital:
        name: Capital
      field.flag:
        name: Flag
      field.flag-similarity:
        name: Flag similarity
    card_template_order:
      - template.flag-country
    card_templates:
      template.flag-country:
        name: Flag - Country
        question_format: '{{Flag}}'
        answer_format: '{{Flag similarity}}'
        adapter_ids: {}
    styling: ''
    adapter_ids: {}
notes:
  note.finland:
    note_type_id: note-type.country
    fields:
      field.capital: Helsinki
      field.country: Finland
      field.flag: '<img src="flags/fi.png">'
      field.flag-similarity:
        message:
          - ref: notes.note.iceland.fields.field.country
          - literal: ' ('
          - text: blue background with a white cross
          - literal: '), '
          - ref: notes.note.norway.fields.field.country
          - literal: ' ('
          - text: red background with a blue cross
          - literal: ')'
    tags: []
    adapter_ids: {}
  note.iceland:
    note_type_id: note-type.country
    fields:
      field.capital: Reykjavik
      field.country: Iceland
      field.flag: '<img src="is.png">'
      field.flag-similarity: ''
    tags: []
    adapter_ids: {}
  note.norway:
    note_type_id: note-type.country
    fields:
      field.capital: Oslo
      field.country: Norway
      field.flag: '<img src="no.png">'
      field.flag-similarity: ''
    tags: []
    adapter_ids: {}
media: {}
tombstones: []
"#,
    )
    .unwrap();
    fs::write(
        dir.join("nb.yaml"),
        r#"id: overlay.translation.nb
kind: translation
translations:
  ignore_paths:
    - deck.*
    - note_types.*
    - notes.*.fields.field.country
    - notes.*.fields.field.capital
    - notes.*.fields.field.flag
    - notes.*.tags.*
  direct:
    Iceland: Island
    Norway: Norge
    red background with a blue cross: rød bakgrunn med blått kors
  contextual:
    notes.note.finland:
      blue background with a white cross: blå bakgrunn med hvitt kors
"#,
    )
    .unwrap();
    fs::write(
        dir.join("brainbrew.yaml"),
        r#"base: deck.yaml
overlays:
  overlay.translation.nb:
    file: nb.yaml
    kind: translation
targets:
  nb-standard:
    overlays:
      - overlay.translation.nb
"#,
    )
    .unwrap();
}

fn write_include_workspace(dir: &Path) {
    fs::create_dir_all(dir.join("content")).unwrap();
    fs::create_dir_all(dir.join("templates")).unwrap();
    fs::create_dir_all(dir.join("styles")).unwrap();
    fs::create_dir_all(dir.join("overlays")).unwrap();
    fs::write(
        dir.join("content/base-description.md"),
        "Base deck description.\nWritten in Markdown.\n",
    )
    .unwrap();
    fs::write(
        dir.join("content/overlay-description.md"),
        "Overlay description from Markdown.\n",
    )
    .unwrap();
    fs::write(
        dir.join("templates/front.html"),
        "<section class=\"front\">{{Country}}</section>\n",
    )
    .unwrap();
    fs::write(
        dir.join("templates/back.html"),
        "{{FrontSide}}\n<hr id=\"answer\">\n<section class=\"back\">{{Capital}}</section>\n",
    )
    .unwrap();
    fs::write(
        dir.join("styles/card.css"),
        ".card {\n  font-family: sans-serif;\n}\n",
    )
    .unwrap();
    fs::write(
        dir.join("deck.yaml"),
        r#"deck:
  id: deck.include-fixture
  name: Include Fixture
  description: !include ./content/../content/base-description.md
  adapter_ids:
    crowdanki:uuid: include-fixture-deck-uuid
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.country
      - field.capital
    fields:
      field.capital:
        name: Capital
      field.country:
        name: Country
    card_template_order:
      - template.country-capital
    card_templates:
      template.country-capital:
        name: Country - Capital
        question_format: !include templates/front.html
        answer_format: !include templates/back.html
        adapter_ids: {}
    styling: !include styles/card.css
    adapter_ids:
      crowdanki:uuid: include-fixture-note-type-uuid
notes:
  note.finland:
    note_type_id: note-type.country
    fields:
      field.capital: Helsinki
      field.country: Finland
    tags:
      - Europe
    adapter_ids:
      crowdanki:guid: include-fixture-note-guid
media: {}
tombstones: []
"#,
    )
    .unwrap();
    fs::write(
        dir.join("overlays/description.yaml"),
        r#"id: overlay.patch.description
kind: patch
deck:
  description:
    intent: replace
    value: !include content/overlay-description.md
    expected_base:
      value: |
        Base deck description.
        Written in Markdown.
"#,
    )
    .unwrap();
    fs::write(
        dir.join("brainbrew.yaml"),
        r#"base: deck.yaml
overlays:
  overlay.patch.description:
    file: overlays/description.yaml
    kind: patch
targets:
  base:
    overlays: []
  localized:
    overlays:
      - overlay.patch.description
"#,
    )
    .unwrap();
}

fn write_tar_gz(path: &Path, root_name: &str, source_dir: &Path) {
    let file = fs::File::create(path).unwrap();
    let encoder = GzEncoder::new(file, Compression::default());
    let mut archive = Builder::new(encoder);
    archive.append_dir_all(root_name, source_dir).unwrap();
    let encoder = archive.into_inner().unwrap();
    encoder.finish().unwrap();
}

fn include_directives(input: &str) -> Vec<String> {
    input
        .lines()
        .filter(|line| line.contains("!include"))
        .map(|line| line.trim().to_owned())
        .collect()
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

const CAPITAL_OVERLAY_YAML: &str = r#"id: overlay.patch.capital
kind: patch
notes:
  note.finland:
    intent: merge
    fields:
      field.capital:
        intent: replace
        value: Helsingfors
        expected_base:
          value: Helsinki
"#;

const MESSY_OVERLAY_YAML: &str = r#"kind: patch
id: overlay.patch.capital
notes:
  note.finland:
    fields:
      field.capital:
        expected_base:
          value: Helsinki
        value: Helsingfors
        intent: replace
    intent: merge
"#;

const NOOP_OVERLAY_YAML: &str = r#"id: overlay.noop
kind: patch
"#;

const SECOND_CAPITAL_OVERLAY_YAML: &str = r#"id: overlay.patch.capital.second
kind: patch
notes:
  note.finland:
    intent: merge
    fields:
      field.capital:
        intent: replace
        value: Helsinki City
        expected_base:
          value: Helsinki
"#;

const MANIFEST_YAML: &str = r#"base: deck.yaml
overlays:
  noop.after-capital:
    file: noop.yaml
    kind: patch
    depends_on:
      - patch.capital
  patch.capital:
    file: capital.yaml
    kind: patch
targets:
  patched-via-dependency:
    overlays:
      - noop.after-capital
"#;

const MANIFEST_WITH_EXPORTS_YAML: &str = r#"base: deck.yaml
overlays:
  noop.after-capital:
    file: noop.yaml
    kind: patch
    depends_on:
      - patch.capital
  patch.capital:
    file: capital.yaml
    kind: patch
targets:
  patched-via-dependency:
    overlays:
      - noop.after-capital
    exports:
      crowdanki:
        out: configured-crowdanki
        golden: goldens/patched/deck.json
"#;

const MANIFEST_WITH_PACKAGE_YAML: &str = r#"package:
  id: anki-geo.ultimate-geography
  version: 0.1.0
  compatible_base_versions:
    - '>=0.1,<0.2'
  depends_on:
    - anki-geo.shared-geography
base: deck.yaml
overlays:
  noop.after-capital:
    file: noop.yaml
    kind: patch
    depends_on:
      - patch.capital
  patch.capital:
    file: capital.yaml
    kind: patch
targets:
  patched-via-dependency:
    overlays:
      - noop.after-capital
"#;

const CONFLICT_MANIFEST_YAML: &str = r#"base: deck.yaml
overlays:
  patch.capital:
    file: capital.yaml
    kind: patch
  patch.capital.second:
    file: second.yaml
    kind: patch
targets:
  conflict:
    overlays:
      - patch.capital
      - patch.capital.second
"#;

const SIMPLE_MEDIA_MANIFEST_YAML: &str = r#"base: deck.yaml
overlays: {}
targets:
  base:
    overlays: []
"#;

const MESSY_MANIFEST_YAML: &str = r#"targets:
  patched-via-dependency:
    overlays: [noop.after-capital]
overlays:
  patch.capital:
    kind: patch
    file: capital.yaml
  noop.after-capital:
    depends_on: [patch.capital]
    kind: patch
    file: noop.yaml
base: deck.yaml
"#;

const LOCK_YAML: &str = r#"version: 1
packages:
  anki-geo.ultimate-geography:
    manifest: brainbrew.yaml
    package:
      version: 0.1.0
    original:
      type: git
      url: https://github.com/anki-geo/ultimate-geography.git
      ref: main
    locked:
      type: git
      url: https://github.com/anki-geo/ultimate-geography.git
      rev: ccf150a1b21e
      nar_hash: sha256-example
"#;

const MESSY_LOCK_YAML: &str = r#"packages:
  anki-geo.ultimate-geography:
    locked:
      nar_hash: sha256-example
      rev: ccf150a1b21e
      url: https://github.com/anki-geo/ultimate-geography.git
      type: git
    original:
      ref: main
      url: https://github.com/anki-geo/ultimate-geography.git
      type: git
    package:
      version: 0.1.0
    manifest: brainbrew.yaml
version: 1
"#;

const MESSY_CANONICAL_YAML: &str = r#"deck:
  description: A geography deck fixture.
  id: deck.ultimate-geography
  name: Ultimate Geography
  adapter_ids:
    crowdanki:uuid: 43c5ba66-9a65-11e8-90c9-a0481cc15658
note_types:
  note-type.country:
    adapter_ids:
      crowdanki:uuid: aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa
    name: Country
    styling: |
      .card { font-family: sans-serif; }
    field_order: [field.country, field.capital, field.flag]
    fields:
      field.flag: { name: Flag }
      field.capital: { name: Capital }
      field.country: { name: Country }
    card_template_order: [template.country-capital]
    card_templates:
      template.country-capital:
        adapter_ids: {}
        name: Country - Capital
        question_format: '{{Country}}'
        answer_format: '{{FrontSide}}<hr id=answer>{{Capital}}'
notes:
  note.finland:
    adapter_ids:
      crowdanki:guid: ug-finland-guid
    fields:
      field.flag: '<img src="flags/fi.png">'
      field.capital: Helsinki
      field.country: Finland
    note_type_id: note-type.country
    tags: [Europe, Nordic]
media:
  media.flags-fi-png:
    path: flags/fi.png
    sha256: ''
tombstones: []
"#;

const SAMPLE_WITHOUT_NOTES_CANONICAL_YAML: &str = r#"deck:
  id: deck.ultimate-geography
  name: Ultimate Geography
  description: A geography deck fixture.
  adapter_ids:
    crowdanki:uuid: 43c5ba66-9a65-11e8-90c9-a0481cc15658
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.country
      - field.capital
      - field.flag
    fields:
      field.capital:
        name: Capital
      field.country:
        name: Country
      field.flag:
        name: Flag
    card_template_order:
      - template.country-capital
    card_templates:
      template.country-capital:
        name: Country - Capital
        question_format: '{{Country}}'
        answer_format: '{{FrontSide}}<hr id=answer>{{Capital}}'
        adapter_ids: {}
    styling: |
      .card { font-family: sans-serif; }
    adapter_ids:
      crowdanki:uuid: aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa
notes: {}
media:
  media.flags-fi-png:
    path: flags/fi.png
    sha256: ''
tombstones: []
"#;

const MEDIA_CANONICAL_YAML: &str = r#"deck:
  id: deck.media-fixture
  name: Media Fixture
  description: A deck with media.
  adapter_ids:
    crowdanki:uuid: media-deck-uuid
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.country
      - field.flag
    fields:
      field.country:
        name: Country
      field.flag:
        name: Flag
    card_template_order:
      - template.country-flag
    card_templates:
      template.country-flag:
        name: Country - Flag
        question_format: '{{Country}}'
        answer_format: '{{FrontSide}}<hr id=answer>{{Flag}}'
        adapter_ids: {}
    styling: |
      .card { font-family: sans-serif; }
    adapter_ids:
      crowdanki:uuid: media-note-type-uuid
notes:
  note.finland:
    note_type_id: note-type.country
    fields:
      field.country: Finland
      field.flag: '<img src="flags/fi.png">'
    tags:
      - Media
    adapter_ids:
      crowdanki:guid: media-fi-guid
media:
  media.flags-fi-png:
    path: flags/fi.png
    sha256: 14873f4faae48052921f9272d948a369f775b2406e57a9b8d55fb94452b73948
tombstones: []
"#;

const SAMPLE_CANONICAL_YAML: &str = r#"deck:
  id: deck.ultimate-geography
  name: Ultimate Geography
  description: A geography deck fixture.
  adapter_ids:
    crowdanki:uuid: 43c5ba66-9a65-11e8-90c9-a0481cc15658
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.country
      - field.capital
      - field.flag
    fields:
      field.capital:
        name: Capital
      field.country:
        name: Country
      field.flag:
        name: Flag
    card_template_order:
      - template.country-capital
    card_templates:
      template.country-capital:
        name: Country - Capital
        question_format: '{{Country}}'
        answer_format: '{{FrontSide}}<hr id=answer>{{Capital}}'
        adapter_ids: {}
    styling: |
      .card { font-family: sans-serif; }
    adapter_ids:
      crowdanki:uuid: aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa
notes:
  note.finland:
    note_type_id: note-type.country
    fields:
      field.capital: Helsinki
      field.country: Finland
      field.flag: '<img src="flags/fi.png">'
    tags:
      - Europe
      - Nordic
    adapter_ids:
      crowdanki:guid: ug-finland-guid
media:
  media.flags-fi-png:
    path: flags/fi.png
    sha256: ''
tombstones: []
"#;
