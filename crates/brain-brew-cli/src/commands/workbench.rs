use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{StatusCode, Uri, header};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use brain_brew_core::{
    CanonicalDeck, CardTemplate, Note, NoteType, Overlay, StableId, TranslationCoverageCategory,
    TranslationCoverageEntry, TranslationDictionary,
};
use brain_brew_formats::canonical_yaml;
use brain_brew_formats::manifest::{FederatedDeckManifest, LanguageManifestEntry};
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};

use crate::help;
use crate::io::{manifest_root, plan_manifest_target_with_packages, read_manifest};

static EMBEDDED_WORKBENCH_ASSETS: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/assets/workbench");

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        print!(
            "{}",
            help::command("workbench").expect("workbench help exists")
        );
        return Ok(());
    }

    let Some(subcommand) = args.first().map(String::as_str) else {
        return Err(help::usage_error(
            "workbench",
            "usage: brainbrew workbench serve --manifest brainbrew.yaml",
        ));
    };
    if subcommand == "serve"
        && args
            .get(1)
            .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        print!(
            "{}",
            help::command("workbench").expect("workbench help exists")
        );
        return Ok(());
    }
    match subcommand {
        "serve" => serve_sync(parse_serve_args(&args[1..])?),
        "--help" | "-h" => {
            print!(
                "{}",
                help::command("workbench").expect("workbench help exists")
            );
            Ok(())
        }
        other => Err(format!("unexpected workbench subcommand {other:?}")),
    }
}

#[derive(Clone, Debug)]
struct ServeArgs {
    manifest_path: PathBuf,
    port: u16,
    open_browser: bool,
    dev_assets: Option<PathBuf>,
}

fn parse_serve_args(args: &[String]) -> Result<ServeArgs, String> {
    let mut parsed = ServeArgs {
        manifest_path: PathBuf::from("brainbrew.yaml"),
        port: 0,
        open_browser: true,
        dev_assets: None,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--manifest requires a path".to_owned());
                };
                parsed.manifest_path = PathBuf::from(path);
                index += 2;
            }
            "--port" => {
                let Some(port) = args.get(index + 1) else {
                    return Err("--port requires a port number".to_owned());
                };
                parsed.port = port
                    .parse::<u16>()
                    .map_err(|_| format!("invalid --port value {port:?}"))?;
                index += 2;
            }
            "--no-open" => {
                parsed.open_browser = false;
                index += 1;
            }
            "--dev-assets" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--dev-assets requires a directory".to_owned());
                };
                parsed.dev_assets = Some(PathBuf::from(path));
                index += 2;
            }
            other => return Err(format!("unexpected workbench serve argument {other:?}")),
        }
    }
    if let Some(path) = &parsed.dev_assets
        && !path.is_dir()
    {
        return Err(format!(
            "--dev-assets directory {} does not exist",
            path.display()
        ));
    }
    Ok(parsed)
}

fn serve_sync(args: ServeArgs) -> Result<(), String> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|error| format!("failed to start Tokio runtime: {error}"))?;
    runtime.block_on(serve(args))
}

async fn serve(args: ServeArgs) -> Result<(), String> {
    let manifest = read_manifest(&args.manifest_path)?;
    let metadata = Arc::new(WorkspaceMetadata::load(&args.manifest_path, manifest));
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), args.port))
        .await
        .map_err(|error| format!("failed to bind workbench server: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("failed to inspect workbench server address: {error}"))?;
    let url = format!("http://{address}");

    println!("Workbench listening at {url}");
    io::stdout()
        .flush()
        .map_err(|error| format!("failed to flush workbench URL: {error}"))?;

    if args.open_browser {
        open_browser(&url);
    }

    axum::serve(listener, app(metadata, args.dev_assets))
        .await
        .map_err(|error| format!("workbench server failed: {error}"))
}

fn app(metadata: Arc<WorkspaceMetadata>, dev_assets: Option<PathBuf>) -> Router {
    let router = Router::new()
        .route("/api/health", get(health))
        .route("/api/workspace", get(workspace))
        .route("/api/workbench/note-pivot", get(note_pivot))
        .route("/api/workbench/apply-preview", post(apply_preview))
        .route("/api/workbench/apply", post(apply_edits))
        .route("/api/media/{*path}", get(media_asset))
        .with_state(metadata);

    if let Some(assets) = dev_assets {
        router.fallback_service(
            ServeDir::new(&assets).not_found_service(ServeFile::new(assets.join("index.html"))),
        )
    } else {
        router.fallback(get(embedded_asset))
    }
}

async fn health(State(metadata): State<Arc<WorkspaceMetadata>>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "manifest": metadata.manifest_path.display().to_string(),
    }))
}

async fn workspace(
    State(metadata): State<Arc<WorkspaceMetadata>>,
) -> Result<Json<Value>, (StatusCode, String)> {
    metadata
        .workspace_json()
        .map(Json)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))
}

async fn note_pivot(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Query(query): Query<NotePivotQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    metadata
        .note_pivot_json(&query)
        .map(Json)
        .map_err(workbench_api_error)
}

async fn apply_preview(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Json(request): Json<ApplyRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    metadata
        .apply_request_json(request, ApplyMode::Preview)
        .map(Json)
        .map_err(workbench_api_error)
}

async fn apply_edits(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Json(request): Json<ApplyRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    metadata
        .apply_request_json(request, ApplyMode::Write)
        .map(Json)
        .map_err(workbench_api_error)
}

async fn media_asset(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    AxumPath(path): AxumPath<String>,
) -> Result<Response, (StatusCode, String)> {
    metadata.media_response(&path).map_err(workbench_api_error)
}

fn workbench_api_error(error: String) -> (StatusCode, String) {
    let status = if error.starts_with("invalid ")
        || error.starts_with("missing ")
        || error.starts_with("no ")
        || error.starts_with("unknown ")
    {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    (status, error)
}

async fn embedded_asset(uri: Uri) -> Response {
    let requested_path = uri.path().trim_start_matches('/');
    let asset_path = if requested_path.is_empty() {
        "index.html"
    } else {
        requested_path
    };

    if let Some(file) = EMBEDDED_WORKBENCH_ASSETS.get_file(asset_path) {
        return embedded_file_response(asset_path, file.contents());
    }

    let index = EMBEDDED_WORKBENCH_ASSETS
        .get_file("index.html")
        .expect("embedded workbench index.html exists");
    embedded_file_response("index.html", index.contents())
}

fn embedded_file_response(path: &str, contents: &'static [u8]) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, embedded_content_type(path))
        .body(Body::from(contents))
        .expect("embedded asset response is valid")
}

fn embedded_content_type(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

#[derive(Debug)]
struct WorkspaceMetadata {
    manifest_path: PathBuf,
    manifest_root: PathBuf,
    manifest: FederatedDeckManifest,
}

impl WorkspaceMetadata {
    fn load(manifest_path: &Path, manifest: FederatedDeckManifest) -> Self {
        Self {
            manifest_path: manifest_path.to_path_buf(),
            manifest_root: manifest_root(manifest_path),
            manifest,
        }
    }

    fn workspace_json(&self) -> Result<Value, String> {
        let fingerprints =
            file_fingerprints(&self.manifest_path, &self.manifest_root, &self.manifest)?;
        Ok(json!({
            "manifest": self.manifest_path.display().to_string(),
            "manifest_root": self.manifest_root.display().to_string(),
            "languages": languages_json(&self.manifest),
            "target_labels": target_labels_json(&self.manifest),
            "targets": targets_json(&self.manifest),
            "translation_profile": {
                "structural_fields": self.manifest.translation_profile.structural_fields,
                "optional_paths": self.manifest.translation_profile.optional_paths,
            },
            "fingerprints": fingerprints,
        }))
    }

    fn note_pivot_json(&self, query: &NotePivotQuery) -> Result<Value, String> {
        let context = self.selected_translation_context(
            query.language.as_deref(),
            query.target.as_deref(),
            query.overlay.as_deref(),
        )?;
        Ok(note_pivot_json_from_context(
            &context,
            &self.manifest,
            query.filter.as_deref(),
            query.note.as_deref(),
        ))
    }

    fn apply_request_json(&self, request: ApplyRequest, mode: ApplyMode) -> Result<Value, String> {
        let context = self.selected_translation_context(
            request.language.as_deref(),
            request.target.as_deref(),
            request.overlay.as_deref(),
        )?;
        if request.edits.is_empty() {
            return Err("missing staged edits".to_owned());
        }

        let mut overlay = read_overlay_for_rewrite(&context.selection.overlay_file)?;
        let changes = apply_staged_edits_to_overlay(&mut overlay, &request.edits, &context)?;
        let validation = validate_modified_overlay(&context, &overlay);
        if mode == ApplyMode::Write && !validation.ok {
            return Err("validation failed; preview before applying".to_owned());
        }
        if mode == ApplyMode::Write {
            fs::write(
                &context.selection.overlay_file,
                canonical_yaml::overlay_to_string(&overlay),
            )
            .map_err(|error| {
                format!(
                    "failed to write translation overlay {}: {error}",
                    context.selection.overlay_file.display()
                )
            })?;
        }

        Ok(json!({
            "mode": mode.as_str(),
            "applied": mode == ApplyMode::Write,
            "language": context.selection.language_code,
            "target_label": context.selection.target_label,
            "target_id": context.selection.target_id,
            "overlay_label": context.selection.overlay_label,
            "overlay_id": context.selection.overlay_id,
            "affected_files": [{
                "path": context.selection.overlay_display_file,
                "absolute_path": context.selection.overlay_file.display().to_string(),
            }],
            "changed_entries": changes,
            "validation": {
                "ok": validation.ok,
                "errors": validation.errors,
            },
        }))
    }

    fn media_response(&self, requested_path: &str) -> Result<Response, String> {
        let requested_path = safe_media_relative_path(requested_path)?;
        if !self.media_path_declared(&requested_path)? {
            return Err(format!("unknown media asset {requested_path:?}"));
        }

        let candidates = [
            self.manifest_root.join(&requested_path),
            self.manifest_root.join("media").join(&requested_path),
        ];
        let path = candidates
            .iter()
            .find(|path| path.is_file())
            .ok_or_else(|| format!("missing media asset {requested_path:?}"))?;
        let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, media_content_type(&requested_path))
            .body(Body::from(bytes))
            .map_err(|error| format!("failed to build media response: {error}"))
    }

    fn media_path_declared(&self, requested_path: &str) -> Result<bool, String> {
        for target_id in self.manifest.targets.keys() {
            let plan =
                plan_manifest_target_with_packages(&self.manifest_path, target_id, &[], &[])?;
            let mut current = plan.base;
            if current
                .media
                .values()
                .any(|media| media.path == requested_path)
            {
                return Ok(true);
            }
            for (planned, overlay) in plan.overlays {
                current = if overlay.translations.is_some() {
                    compose_lenient_translation_overlay(&current, &overlay)?
                } else {
                    current.compose(&[overlay]).map_err(|error| {
                        format!("failed to compose overlay {}: {error}", planned.id)
                    })?
                };
                if current
                    .media
                    .values()
                    .any(|media| media.path == requested_path)
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn selected_translation_context(
        &self,
        language: Option<&str>,
        target_label: Option<&str>,
        overlay_label: Option<&str>,
    ) -> Result<SelectedTranslationContext, String> {
        let selection = self.select_translation_target(language, target_label, overlay_label)?;
        let plan = plan_manifest_target_with_packages(
            &self.manifest_path,
            &selection.target_id,
            &[],
            &[],
        )?;
        let mut current = plan.base.clone();
        let mut selected_source_deck = None;
        let mut selected_report = None;
        let mut selected_display_file = selection.overlay_display_file.clone();
        let mut selected_file = selection.overlay_file.clone();

        for (planned, overlay) in &plan.overlays {
            if planned.id == selection.overlay_id {
                selected_display_file = planned.display_file.clone();
                selected_file = planned.file.clone();
                selected_source_deck = Some(current.clone());
                selected_report = current.translation_coverage(overlay);
            }
            current = if overlay.translations.is_some() {
                compose_lenient_translation_overlay(&current, overlay)?
            } else {
                current
                    .compose(std::slice::from_ref(overlay))
                    .map_err(|error| format!("failed to compose overlay {}: {error}", planned.id))?
            };
        }

        let Some(source_deck) = selected_source_deck else {
            return Err(format!(
                "target {} does not include translation overlay {}",
                selection.target_id, selection.overlay_id
            ));
        };
        let Some(report) = selected_report else {
            return Err(format!(
                "overlay {} is not a translation overlay",
                selection.overlay_id
            ));
        };
        Ok(SelectedTranslationContext {
            selection: WorkbenchSelection {
                overlay_file: selected_file,
                overlay_display_file: selected_display_file,
                ..selection
            },
            base_deck: plan.base,
            plan_overlays: plan.overlays,
            source_deck,
            target_deck: current,
            report,
        })
    }

    fn select_translation_target(
        &self,
        language: Option<&str>,
        target_label: Option<&str>,
        overlay_label: Option<&str>,
    ) -> Result<WorkbenchSelection, String> {
        let (language_code, language_entry) = if let Some(language) = language {
            let entry = self
                .manifest
                .languages
                .get(language)
                .ok_or_else(|| format!("unknown language {language:?}"))?;
            (language.to_owned(), entry)
        } else {
            self.manifest
                .languages
                .iter()
                .find(|(_, entry)| !entry.source && !entry.translation_overlays.is_empty())
                .map(|(code, entry)| (code.clone(), entry))
                .ok_or_else(|| {
                    "no target language with translation overlays is configured".to_owned()
                })?
        };
        if language_entry.source {
            return Err(format!(
                "invalid source language {language_code:?}; choose a target language"
            ));
        }
        if language_entry.translation_overlays.is_empty() {
            return Err(format!(
                "language {language_code:?} has no translation overlays"
            ));
        }

        let target_label = target_label
            .map(str::to_owned)
            .or_else(|| {
                (!language_entry.primary_target.is_empty())
                    .then(|| language_entry.primary_target.clone())
            })
            .or_else(|| language_entry.targets.keys().next().cloned())
            .ok_or_else(|| format!("language {language_code:?} has no targets"))?;
        let target_id = language_entry
            .targets
            .get(&target_label)
            .ok_or_else(|| {
                format!("language {language_code:?} has no target label {target_label:?}")
            })?
            .clone();

        let overlay_label = overlay_label
            .map(str::to_owned)
            .or_else(|| {
                language_entry
                    .translation_overlays
                    .contains_key("base")
                    .then(|| "base".to_owned())
            })
            .or_else(|| language_entry.translation_overlays.keys().next().cloned())
            .ok_or_else(|| format!("language {language_code:?} has no translation overlays"))?;
        let overlay_id = language_entry
            .translation_overlays
            .get(&overlay_label)
            .ok_or_else(|| {
                format!("language {language_code:?} has no overlay label {overlay_label:?}")
            })?
            .clone();
        let overlay_file = self
            .manifest
            .overlays
            .get(&overlay_id)
            .map(|overlay| self.manifest_root.join(&overlay.file))
            .ok_or_else(|| format!("overlay {overlay_id:?} is not in the manifest catalog"))?;

        let overlay_badges = language_entry
            .translation_overlays
            .iter()
            .map(|(label, id)| OverlayBadge {
                label: label.clone(),
                id: id.clone(),
                active: label == &overlay_label,
            })
            .collect();

        Ok(WorkbenchSelection {
            language_code,
            language_display_name: language_entry.display_name.clone(),
            target_label,
            target_id,
            overlay_label,
            overlay_id,
            overlay_file,
            overlay_display_file: String::new(),
            overlay_badges,
            structural_fields: self
                .manifest
                .translation_profile
                .structural_fields
                .iter()
                .cloned()
                .collect(),
        })
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
struct NotePivotQuery {
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    filter: Option<String>,
    note: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ApplyRequest {
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    edits: Vec<StagedTranslationEdit>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StagedTranslationEdit {
    path: String,
    source: String,
    value: String,
    #[serde(default = "default_edit_mode")]
    mode: EditMode,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum EditMode {
    Direct,
    Contextual,
    NoChange,
}

fn default_edit_mode() -> EditMode {
    EditMode::Direct
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ApplyMode {
    Preview,
    Write,
}

impl ApplyMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Write => "write",
        }
    }
}

#[derive(Clone, Debug)]
struct WorkbenchSelection {
    language_code: String,
    language_display_name: String,
    target_label: String,
    target_id: String,
    overlay_label: String,
    overlay_id: String,
    overlay_file: PathBuf,
    overlay_display_file: String,
    overlay_badges: Vec<OverlayBadge>,
    structural_fields: BTreeSet<String>,
}

#[derive(Clone, Debug)]
struct OverlayBadge {
    label: String,
    id: String,
    active: bool,
}

#[derive(Clone)]
struct SelectedTranslationContext {
    selection: WorkbenchSelection,
    base_deck: CanonicalDeck,
    plan_overlays: Vec<(crate::io::PlannedOverlay, Overlay)>,
    source_deck: CanonicalDeck,
    target_deck: CanonicalDeck,
    report: brain_brew_core::TranslationCoverageReport,
}

#[derive(Clone, Debug, Serialize)]
struct ApplyValidation {
    ok: bool,
    errors: Vec<String>,
}

fn note_pivot_json_from_context(
    context: &SelectedTranslationContext,
    manifest: &FederatedDeckManifest,
    filter: Option<&str>,
    selected_note: Option<&str>,
) -> Value {
    let entries_by_path = context
        .report
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let source_counts = main_field_source_counts(&context.source_deck, &context.selection, context);
    let progress = main_progress(&context.source_deck, context, &entries_by_path);
    let notes = note_pivot_notes_json(
        &context.source_deck,
        &context.target_deck,
        context,
        &entries_by_path,
        &source_counts,
        filter,
        selected_note,
    );

    json!({
        "language": {
            "code": context.selection.language_code,
            "display_name": context.selection.language_display_name,
        },
        "target": {
            "label": context.selection.target_label,
            "id": context.selection.target_id,
        },
        "overlay": {
            "label": context.selection.overlay_label,
            "id": context.selection.overlay_id,
            "file": context.selection.overlay_display_file,
        },
        "overlay_badges": overlay_badges_json(context),
        "selection_options": selection_options_json(context, manifest),
        "filters": {
            "active": filter.unwrap_or("all"),
            "available": ["all", "missing", "stale", "needs_work"],
        },
        "progress": progress,
        "notes": notes,
        "stale_entries": stale_entries_json(&context.report.entries),
    })
}

fn overlay_badges_json(context: &SelectedTranslationContext) -> Value {
    json!(
        context
            .selection
            .overlay_badges
            .iter()
            .map(|badge| json!({
                "label": badge.label,
                "id": badge.id,
                "active": badge.active,
            }))
            .collect::<Vec<_>>()
    )
}

fn selection_options_json(
    context: &SelectedTranslationContext,
    manifest: &FederatedDeckManifest,
) -> Value {
    let languages = manifest
        .languages
        .iter()
        .filter(|(_, entry)| !entry.source && !entry.translation_overlays.is_empty())
        .map(|(code, entry)| {
            json!({
                "code": code,
                "display_name": entry.display_name,
                "active": code == &context.selection.language_code,
            })
        })
        .collect::<Vec<_>>();

    let Some(language_entry) = manifest.languages.get(&context.selection.language_code) else {
        return json!({
            "languages": languages,
            "targets": [],
            "overlays": [],
        });
    };

    let targets = language_entry
        .targets
        .iter()
        .map(|(label, id)| {
            json!({
                "label": label,
                "id": id,
                "active": label == &context.selection.target_label,
            })
        })
        .collect::<Vec<_>>();
    let overlays = language_entry
        .translation_overlays
        .iter()
        .map(|(label, id)| {
            json!({
                "label": label,
                "id": id,
                "active": label == &context.selection.overlay_label,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "languages": languages,
        "targets": targets,
        "overlays": overlays,
    })
}

fn main_field_source_counts(
    source_deck: &CanonicalDeck,
    _selection: &WorkbenchSelection,
    context: &SelectedTranslationContext,
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for row in main_field_rows(source_deck, &context.selection, &context.report.entries) {
        *counts.entry(row.source).or_insert(0) += 1;
    }
    counts
}

#[derive(Clone, Debug)]
struct MainFieldRow {
    note_id: StableId,
    note_type_id: StableId,
    field_id: StableId,
    field_name: String,
    path: String,
    source: String,
    structural: bool,
    category: TranslationCoverageCategory,
    translated: String,
}

fn main_field_rows(
    source_deck: &CanonicalDeck,
    selection: &WorkbenchSelection,
    entries: &[TranslationCoverageEntry],
) -> Vec<MainFieldRow> {
    let structural_fields = structural_field_set(selection, source_deck);
    let entries_by_path = entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut rows = Vec::new();
    for (note_id, note) in &source_deck.notes {
        let Some(note_type) = source_deck.note_types.get(&note.note_type_id) else {
            continue;
        };
        for field in &note_type.fields {
            let source = note.fields.get(&field.id).cloned().unwrap_or_default();
            if source.is_empty() {
                continue;
            }
            let path = format!("notes.{note_id}.fields.{}", field.id);
            let structural = structural_fields.contains(field.id.as_str());
            let entry = entries_by_path.get(path.as_str()).copied();
            rows.push(MainFieldRow {
                note_id: note_id.clone(),
                note_type_id: note.note_type_id.clone(),
                field_id: field.id.clone(),
                field_name: field.name.clone(),
                path,
                source: source.clone(),
                structural,
                category: entry
                    .map(|entry| entry.category)
                    .unwrap_or(TranslationCoverageCategory::UntranslatedFallback),
                translated: entry
                    .and_then(|entry| entry.translated.clone())
                    .unwrap_or(source),
            });
        }
    }
    rows
}

fn structural_field_set(
    selection: &WorkbenchSelection,
    _source_deck: &CanonicalDeck,
) -> BTreeSet<String> {
    selection.structural_fields.clone()
}

fn main_progress(
    source_deck: &CanonicalDeck,
    context: &SelectedTranslationContext,
    _entries_by_path: &BTreeMap<&str, &TranslationCoverageEntry>,
) -> Value {
    let rows = main_field_rows(source_deck, &context.selection, &context.report.entries)
        .into_iter()
        .filter(|row| !row.structural)
        .collect::<Vec<_>>();
    let total = rows.len();
    let complete = rows
        .iter()
        .filter(|row| {
            matches!(
                row.category,
                TranslationCoverageCategory::DirectTranslation
                    | TranslationCoverageCategory::ContextualOverride
                    | TranslationCoverageCategory::NoChange
            )
        })
        .count();
    let missing = rows
        .iter()
        .filter(|row| row.category == TranslationCoverageCategory::UntranslatedFallback)
        .count();
    let stale = context
        .report
        .entries
        .iter()
        .filter(|entry| is_stale_category(entry.category))
        .count();
    let percent = complete
        .checked_mul(100)
        .and_then(|value| value.checked_div(total))
        .unwrap_or(100);
    json!({
        "complete": complete,
        "total": total,
        "missing": missing,
        "stale": stale,
        "needs_work": missing + stale,
        "percent": percent,
    })
}

fn note_pivot_notes_json(
    source_deck: &CanonicalDeck,
    target_deck: &CanonicalDeck,
    context: &SelectedTranslationContext,
    _entries_by_path: &BTreeMap<&str, &TranslationCoverageEntry>,
    source_counts: &BTreeMap<String, usize>,
    filter: Option<&str>,
    selected_note: Option<&str>,
) -> Value {
    let rows = main_field_rows(source_deck, &context.selection, &context.report.entries);
    let rows_by_note = rows.into_iter().fold(
        BTreeMap::<StableId, Vec<MainFieldRow>>::new(),
        |mut rows_by_note, row| {
            rows_by_note
                .entry(row.note_id.clone())
                .or_default()
                .push(row);
            rows_by_note
        },
    );
    let mut notes = Vec::new();
    for (index, (note_id, note)) in source_deck.notes.iter().enumerate() {
        if selected_note.is_some_and(|selected| selected != note_id.as_str()) {
            continue;
        }
        let field_rows = rows_by_note.get(note_id).cloned().unwrap_or_default();
        let note_entries = context
            .report
            .entries
            .iter()
            .filter(|entry| entry.path.starts_with(&format!("notes.{note_id}.")))
            .cloned()
            .collect::<Vec<_>>();
        if !note_matches_filter(&field_rows, &note_entries, filter) {
            continue;
        }
        let Some(note_type) = source_deck.note_types.get(&note.note_type_id) else {
            continue;
        };
        let target_note = target_deck.notes.get(note_id).unwrap_or(note);
        let fields = field_rows
            .iter()
            .map(|row| {
                let target = target_note
                    .fields
                    .get(&row.field_id)
                    .cloned()
                    .unwrap_or_else(|| row.translated.clone());
                json!({
                    "path": row.path,
                    "note_id": row.note_id.to_string(),
                    "note_type_id": row.note_type_id.to_string(),
                    "field_id": row.field_id.to_string(),
                    "field_name": row.field_name,
                    "source": row.source,
                    "target": target,
                    "status": row.category.as_str(),
                    "occurrence_count": source_counts.get(&row.source).copied().unwrap_or(1),
                    "structural": row.structural,
                    "editable": !row.structural,
                    "context_path": contextual_path_for_row(row, &context.report.entries),
                    "controls": ["direct", "contextual", "no_change"],
                })
            })
            .collect::<Vec<_>>();
        let source_preview = render_note_cards(source_deck, note, note_type);
        let target_note_type = target_deck
            .note_types
            .get(&target_note.note_type_id)
            .unwrap_or(note_type);
        let target_preview = render_note_cards(target_deck, target_note, target_note_type);
        let title = note
            .fields
            .values()
            .next()
            .cloned()
            .unwrap_or_else(|| note_id.to_string());
        notes.push(json!({
            "index": index,
            "note_id": note_id.to_string(),
            "note_type_id": note.note_type_id.to_string(),
            "title": title,
            "status": note_status(&field_rows, &note_entries),
            "fields": fields,
            "source_preview": source_preview,
            "target_preview": target_preview,
        }));
    }
    json!(notes)
}

fn contextual_path_for_row(row: &MainFieldRow, entries: &[TranslationCoverageEntry]) -> String {
    let Some(note_context) = note_context_prefix(&row.path) else {
        return row.path.clone();
    };
    let note_path_prefix = format!("{note_context}.");
    let same_source_in_note = entries
        .iter()
        .filter(|entry| {
            entry.source == row.source
                && (entry.path == note_context || entry.path.starts_with(&note_path_prefix))
        })
        .count();
    if same_source_in_note <= 1 {
        note_context
    } else {
        row.path.clone()
    }
}

fn note_context_prefix(path: &str) -> Option<String> {
    path.split_once(".fields.")
        .map(|(note_context, _)| note_context.to_owned())
}

fn note_matches_filter(
    rows: &[MainFieldRow],
    entries: &[TranslationCoverageEntry],
    filter: Option<&str>,
) -> bool {
    match filter.unwrap_or("all") {
        "missing" => rows.iter().any(|row| {
            !row.structural && row.category == TranslationCoverageCategory::UntranslatedFallback
        }),
        "stale" => entries
            .iter()
            .any(|entry| is_stale_category(entry.category)),
        "needs_work" | "needs-work" => {
            rows.iter().any(|row| {
                !row.structural && row.category == TranslationCoverageCategory::UntranslatedFallback
            }) || entries
                .iter()
                .any(|entry| is_stale_category(entry.category))
        }
        _ => true,
    }
}

fn note_status(rows: &[MainFieldRow], entries: &[TranslationCoverageEntry]) -> &'static str {
    if rows.iter().any(|row| {
        !row.structural && row.category == TranslationCoverageCategory::UntranslatedFallback
    }) {
        "missing"
    } else if entries
        .iter()
        .any(|entry| is_stale_category(entry.category))
    {
        "stale"
    } else {
        "complete"
    }
}

fn stale_entries_json(entries: &[TranslationCoverageEntry]) -> Value {
    json!(
        entries
            .iter()
            .filter(|entry| is_stale_category(entry.category))
            .map(|entry| json!({
                "path": entry.path,
                "source": entry.source,
                "target": entry.translated,
                "status": entry.category.as_str(),
            }))
            .collect::<Vec<_>>()
    )
}

fn is_stale_category(category: TranslationCoverageCategory) -> bool {
    matches!(
        category,
        TranslationCoverageCategory::StaleDirectKey
            | TranslationCoverageCategory::StaleContextualKey
            | TranslationCoverageCategory::StaleNoChangeKey
            | TranslationCoverageCategory::StaleTargetAddition
            | TranslationCoverageCategory::StaleVariableKey
            | TranslationCoverageCategory::StaleAdapterIdKey
            | TranslationCoverageCategory::InvalidTargetAddition
    )
}

fn render_note_cards(deck: &CanonicalDeck, note: &Note, note_type: &NoteType) -> Value {
    let rendered_deck = deck.render_variables().unwrap_or_else(|_| deck.clone());
    let rendered_note = rendered_deck.notes.get(&note.id).unwrap_or(note);
    let rendered_note_type = rendered_deck
        .note_types
        .get(&note_type.id)
        .unwrap_or(note_type);
    let cards = rendered_note_type
        .card_templates
        .iter()
        .map(|template| render_card(rendered_note, rendered_note_type, template))
        .collect::<Vec<_>>();
    json!({
        "styling": rendered_note_type.styling,
        "cards": cards,
    })
}

fn render_card(note: &Note, note_type: &NoteType, template: &CardTemplate) -> Value {
    let question = render_template_side(&template.question_format, note, note_type, "");
    let answer = render_template_side(&template.answer_format, note, note_type, &question);
    json!({
        "template_id": template.id.to_string(),
        "template_name": template.name,
        "question_html": question,
        "answer_html": answer,
    })
}

fn render_template_side(
    template: &str,
    note: &Note,
    note_type: &NoteType,
    front_side: &str,
) -> String {
    let mut rendered = template.replace("{{FrontSide}}", front_side);
    for field in &note_type.fields {
        let value = note.fields.get(&field.id).map(String::as_str).unwrap_or("");
        rendered = render_conditional_sections(&rendered, &field.name, value);
    }
    for field in &note_type.fields {
        let value = note.fields.get(&field.id).map(String::as_str).unwrap_or("");
        let preview_value = format!(
            "<span data-preview-field-id=\"{}\">{}</span>",
            html_attribute_escape(field.id.as_str()),
            value
        );
        rendered = rendered.replace(&format!("{{{{{}}}}}", field.name), &preview_value);
        rendered = rendered.replace(&format!("{{{{type:{}}}}}", field.name), "");
    }
    rewrite_media_sources(&rendered)
}

fn render_conditional_sections(template: &str, field_name: &str, value: &str) -> String {
    let mut rendered = template.to_owned();
    rendered = replace_section(
        &rendered,
        &format!("{{{{#{field_name}}}}}"),
        &format!("{{{{/{field_name}}}}}"),
        !value.is_empty(),
    );
    replace_section(
        &rendered,
        &format!("{{{{^{field_name}}}}}"),
        &format!("{{{{/{field_name}}}}}"),
        value.is_empty(),
    )
}

fn replace_section(template: &str, open: &str, close: &str, keep: bool) -> String {
    let mut output = template.to_owned();
    while let Some(start) = output.find(open) {
        let content_start = start + open.len();
        let Some(close_offset) = output[content_start..].find(close) else {
            break;
        };
        let close_start = content_start + close_offset;
        let close_end = close_start + close.len();
        let replacement = if keep {
            output[content_start..close_start].to_owned()
        } else {
            String::new()
        };
        output.replace_range(start..close_end, &replacement);
    }
    output
}

fn html_attribute_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn rewrite_media_sources(html: &str) -> String {
    rewrite_media_source_quotes(
        &rewrite_media_source_quotes(html, "src=\"", '"'),
        "src='",
        '\'',
    )
}

fn rewrite_media_source_quotes(html: &str, marker: &str, quote: char) -> String {
    let mut output = String::new();
    let mut rest = html;
    while let Some(start) = rest.find(marker) {
        let (before, after_marker) = rest.split_at(start + marker.len());
        output.push_str(before);
        let Some(end) = after_marker.find(quote) else {
            output.push_str(after_marker);
            return output;
        };
        let (path, after_path) = after_marker.split_at(end);
        if is_external_or_already_routed_media(path) {
            output.push_str(path);
        } else {
            output.push_str("/api/media/");
            output.push_str(path.trim_start_matches('/'));
        }
        rest = after_path;
    }
    output.push_str(rest);
    output
}

fn is_external_or_already_routed_media(path: &str) -> bool {
    path.starts_with("/api/media/")
        || path.starts_with("http://")
        || path.starts_with("https://")
        || path.starts_with("data:")
}

fn safe_media_relative_path(path: &str) -> Result<String, String> {
    let path = path.trim_start_matches('/');
    if path.is_empty()
        || path.contains("..")
        || path.starts_with('~')
        || Path::new(path).is_absolute()
    {
        return Err(format!("invalid media path {path:?}"));
    }
    Ok(path.to_owned())
}

fn media_content_type(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("gif") => "image/gif",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

fn read_overlay_for_rewrite(path: &Path) -> Result<Overlay, String> {
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    canonical_yaml::overlay_from_str(&input).map_err(|error| error.to_string())
}

fn editable_sources_by_path(context: &SelectedTranslationContext) -> BTreeMap<String, String> {
    main_field_rows(
        &context.source_deck,
        &context.selection,
        &context.report.entries,
    )
    .into_iter()
    .filter(|row| !row.structural)
    .map(|row| (row.path, row.source))
    .collect()
}

fn contextual_path_for_edit(
    context: &SelectedTranslationContext,
    edit: &StagedTranslationEdit,
) -> Result<String, String> {
    let rows = main_field_rows(
        &context.source_deck,
        &context.selection,
        &context.report.entries,
    );
    rows.iter()
        .find(|row| row.path == edit.path && row.source == edit.source && !row.structural)
        .map(|row| contextual_path_for_row(row, &context.report.entries))
        .ok_or_else(|| format!("invalid contextual edit path {:?}", edit.path))
}

fn apply_staged_edits_to_overlay(
    overlay: &mut Overlay,
    edits: &[StagedTranslationEdit],
    context: &SelectedTranslationContext,
) -> Result<Vec<Value>, String> {
    let editable_sources = editable_sources_by_path(context);
    let translations = overlay.translations.get_or_insert_with(Default::default);
    let mut changed = Vec::new();
    for edit in edits {
        if !edit.path.starts_with("notes.") || !edit.path.contains(".fields.") {
            return Err(format!("invalid note-field edit path {:?}", edit.path));
        }
        if edit.source.is_empty() {
            return Err(format!("invalid empty source for {}", edit.path));
        }
        let Some(expected_source) = editable_sources.get(edit.path.as_str()) else {
            return Err(format!(
                "invalid edit path {:?}; choose an editable note field in the selected target",
                edit.path
            ));
        };
        if expected_source != &edit.source {
            return Err(format!(
                "invalid source {:?} for {}; expected {:?}",
                edit.source, edit.path, expected_source
            ));
        }
        match edit.mode {
            EditMode::Direct => {
                translations.no_change.remove(&edit.source);
                remove_contextual_source_for_path(translations, &edit.path, &edit.source);
                let old = translations
                    .direct
                    .insert(edit.source.clone(), edit.value.clone());
                changed.push(json!({
                    "mode": "direct",
                    "path": edit.path,
                    "source": edit.source,
                    "old": old,
                    "new": edit.value,
                }));
            }
            EditMode::Contextual => {
                translations.no_change.remove(&edit.source);
                let context_path = contextual_path_for_edit(context, edit)?;
                let replacements = translations
                    .contextual
                    .entry(context_path.clone())
                    .or_default();
                let old = replacements.insert(edit.source.clone(), edit.value.clone());
                changed.push(json!({
                    "mode": "contextual",
                    "path": context_path,
                    "field_path": edit.path,
                    "source": edit.source,
                    "old": old,
                    "new": edit.value,
                }));
            }
            EditMode::NoChange => {
                translations.direct.remove(&edit.source);
                remove_contextual_source_for_path(translations, &edit.path, &edit.source);
                let inserted = translations.no_change.insert(edit.source.clone());
                changed.push(json!({
                    "mode": "no_change",
                    "path": edit.path,
                    "source": edit.source,
                    "old": if inserted { Value::Null } else { json!(edit.source) },
                    "new": edit.source,
                }));
            }
        }
    }
    Ok(changed)
}

fn remove_contextual_source_for_path(
    translations: &mut TranslationDictionary,
    path: &str,
    source: &str,
) {
    let contexts = translations
        .contextual
        .iter()
        .filter(|(context_path, replacements)| {
            replacements.contains_key(source)
                && (context_path.as_str() == path || path.starts_with(&format!("{context_path}.")))
        })
        .map(|(context_path, _)| context_path.clone())
        .collect::<Vec<_>>();
    for context_path in contexts {
        if let Some(replacements) = translations.contextual.get_mut(&context_path) {
            replacements.remove(source);
            if replacements.is_empty() {
                translations.contextual.remove(&context_path);
            }
        }
    }
}

fn validate_modified_overlay(
    context: &SelectedTranslationContext,
    modified_overlay: &Overlay,
) -> ApplyValidation {
    match compose_with_modified_overlay(context, modified_overlay) {
        Ok(_) => ApplyValidation {
            ok: true,
            errors: Vec::new(),
        },
        Err(error) => ApplyValidation {
            ok: false,
            errors: vec![error],
        },
    }
}

fn compose_with_modified_overlay(
    context: &SelectedTranslationContext,
    modified_overlay: &Overlay,
) -> Result<CanonicalDeck, String> {
    let mut current = context.base_deck.clone();
    for (planned, overlay) in &context.plan_overlays {
        let active_overlay = if planned.id == context.selection.overlay_id {
            modified_overlay
        } else {
            overlay
        };
        current = if active_overlay.translations.is_some() {
            compose_lenient_translation_overlay(&current, active_overlay)?
        } else {
            current
                .compose(std::slice::from_ref(active_overlay))
                .map_err(|error| format!("failed to compose overlay {}: {error}", planned.id))?
        };
    }
    Ok(current)
}

fn compose_lenient_translation_overlay(
    current: &CanonicalDeck,
    overlay: &Overlay,
) -> Result<CanonicalDeck, String> {
    let mut sanitized = overlay.clone();
    if let Some(translations) = &mut sanitized.translations {
        translations.require_complete = false;
        if let Some(report) = current.translation_coverage(overlay) {
            for entry in report.entries {
                match entry.category {
                    TranslationCoverageCategory::StaleDirectKey => {
                        translations.direct.remove(&entry.source);
                    }
                    TranslationCoverageCategory::StaleContextualKey => {
                        if let Some(context) = &entry.context
                            && let Some(replacements) = translations.contextual.get_mut(context)
                        {
                            replacements.remove(&entry.source);
                            if replacements.is_empty() {
                                translations.contextual.remove(context);
                            }
                        }
                    }
                    TranslationCoverageCategory::StaleNoChangeKey => {
                        translations.no_change.remove(&entry.source);
                    }
                    TranslationCoverageCategory::StaleTargetAddition
                    | TranslationCoverageCategory::InvalidTargetAddition => {
                        let path = entry.context.as_deref().unwrap_or(&entry.path);
                        translations.target_additions.remove(path);
                    }
                    TranslationCoverageCategory::StaleVariableKey => {
                        if let Some(variable_key) = &entry.context
                            && let Some(replacements) = translations.variables.get_mut(variable_key)
                        {
                            replacements.remove(&entry.source);
                            if replacements.is_empty() {
                                translations.variables.remove(variable_key);
                            }
                        }
                    }
                    TranslationCoverageCategory::StaleAdapterIdKey => {
                        if let Some(adapter_key) = &entry.context
                            && let Some(replacements) =
                                translations.adapter_ids.get_mut(adapter_key)
                        {
                            replacements.remove(&entry.source);
                            if replacements.is_empty() {
                                translations.adapter_ids.remove(adapter_key);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    current.compose(&[sanitized]).map_err(|error| {
        format!(
            "failed to compose translation overlay {}: {error}",
            overlay.id
        )
    })
}

fn languages_json(manifest: &FederatedDeckManifest) -> Value {
    let languages = manifest
        .languages
        .iter()
        .map(|(code, language)| (code.clone(), language_json(language)))
        .collect::<serde_json::Map<_, _>>();
    Value::Object(languages)
}

fn language_json(language: &LanguageManifestEntry) -> Value {
    json!({
        "display_name": language.display_name,
        "source": language.source,
        "translation_overlays": language.translation_overlays,
        "primary_target": language.primary_target,
        "targets": language.targets,
    })
}

fn target_labels_json(manifest: &FederatedDeckManifest) -> Value {
    let mut labels = BTreeMap::<String, Vec<Value>>::new();
    for (language, entry) in &manifest.languages {
        for (label, target) in &entry.targets {
            labels.entry(target.clone()).or_default().push(json!({
                "language": language,
                "label": label,
            }));
        }
    }
    json!(labels)
}

fn targets_json(manifest: &FederatedDeckManifest) -> Value {
    let targets = manifest
        .targets
        .iter()
        .map(|(name, target)| {
            (
                name.clone(),
                json!({
                    "extends": target.extends,
                    "overlays": target.overlays,
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    Value::Object(targets)
}

fn file_fingerprints(
    manifest_path: &Path,
    root: &Path,
    manifest: &FederatedDeckManifest,
) -> Result<Vec<Value>, String> {
    let mut files = vec![manifest_path.to_path_buf(), root.join(&manifest.base)];
    for overlay in manifest.overlays.values() {
        files.push(root.join(&overlay.file));
    }
    files.sort();
    files.dedup();
    files
        .into_iter()
        .map(|path| file_fingerprint(root, &path))
        .collect()
}

fn file_fingerprint(root: &Path, path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let hash = Sha256::digest(&bytes);
    Ok(json!({
        "path": workspace_path(root, path),
        "sha256": format!("{hash:x}"),
    }))
}

fn workspace_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(url);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    };

    let _ = command.spawn();
}
