#![cfg(target_arch = "wasm32")]

use leptos::prelude::*;
use serde_json::Value;

#[derive(Clone, Copy)]
pub struct Staging {
    revision: RwSignal<u64>,
}

impl Staging {
    pub fn new() -> Self {
        Self {
            revision: RwSignal::new(0),
        }
    }

    pub fn bump(self) {
        self.revision.update(|revision| {
            *revision = revision.saturating_add(1);
        });
    }
}

pub fn provide(staging: Staging) {
    provide_context(staging);
}

pub fn current() -> Option<Staging> {
    use_context::<Staging>()
}

pub fn staged_translation_for_parts(prefix: &str, path: &str, source: &str) -> Option<Value> {
    read_json(&translation_key(prefix, path, source))
}

pub fn staged_source_for_parts(prefix: &str, path: &str, source: &str) -> Option<Value> {
    read_json(&source_key(prefix, path, source))
}

pub fn stage_translation(
    prefix: &str,
    path: &str,
    storage_source: &str,
    effective_source: &str,
    value: &str,
    mode: &str,
) {
    let mut edit = serde_json::json!({
        "kind": "translation",
        "path": path,
        "source": effective_source,
        "value": value,
        "mode": mode,
    });
    if mode == "contextual" {
        edit["context_path"] = Value::String(path.to_owned());
    }
    write_json(&translation_key(prefix, path, storage_source), &edit);
}

pub fn stage_source_edit(
    prefix: &str,
    path: &str,
    source: &str,
    value: &str,
    scope: &str,
    impact: &str,
) {
    let edit = serde_json::json!({
        "kind": "source",
        "path": path,
        "source": source,
        "value": value,
        "scope": scope,
        "impact_action": impact,
    });
    write_json(&source_key(prefix, path, source), &edit);
    if let Some(mut translation_edit) = read_json(&translation_key(prefix, path, source)) {
        translation_edit["source"] = Value::String(value.to_owned());
        write_json(&translation_key(prefix, path, source), &translation_edit);
    }
}

pub fn collect_staged_edits_for_prefixes(prefixes: &[String]) -> Vec<Value> {
    let Some(storage) = local_storage() else {
        return Vec::new();
    };
    let mut edits = Vec::new();
    let length = storage.length().unwrap_or(0);
    for index in 0..length {
        let Some(key) = storage.key(index).ok().flatten() else {
            continue;
        };
        if !prefixes.iter().any(|prefix| {
            key.starts_with(&format!("{prefix}translation::"))
                || key.starts_with(&format!("{prefix}source::"))
        }) {
            continue;
        }
        if let Some(value) = storage.get_item(&key).ok().flatten()
            && let Ok(edit) = serde_json::from_str::<Value>(&value)
        {
            edits.push(edit);
        }
    }
    edits
}

pub fn staged_count_for_prefixes(prefixes: &[String]) -> usize {
    local_storage().map_or(0, |storage| {
        (0..storage.length().unwrap_or(0))
            .filter_map(|index| storage.key(index).ok().flatten())
            .filter(|key| prefixes.iter().any(|prefix| key.starts_with(prefix)))
            .count()
    })
}

pub fn clear_prefix(prefix: &str) {
    let Some(storage) = local_storage() else {
        return;
    };
    let keys = (0..storage.length().unwrap_or(0))
        .filter_map(|index| storage.key(index).ok().flatten())
        .filter(|key| key.starts_with(prefix))
        .collect::<Vec<_>>();
    for key in keys {
        let _ = storage.remove_item(&key);
    }
    bump_current();
}

pub fn storage_prefix_for_parts(language: &str, target: &str, overlay: &str) -> String {
    format!("brainbrew.workbench.staged.{language}.{target}.{overlay}::")
}

pub fn translation_key(prefix: &str, path: &str, source: &str) -> String {
    format!("{prefix}translation::{path}::{source}")
}

pub fn source_key(prefix: &str, path: &str, source: &str) -> String {
    format!("{prefix}source::{path}::{source}")
}

pub fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|window| window.local_storage().ok().flatten())
}

pub fn bump_current() {
    if let Some(staging) = current() {
        staging.bump();
    }
}

fn read_json(key: &str) -> Option<Value> {
    local_storage()
        .and_then(|storage| storage.get_item(key).ok().flatten())
        .and_then(|value| serde_json::from_str(&value).ok())
}

fn write_json(key: &str, value: &Value) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(key, &value.to_string());
    }
    bump_current();
}
