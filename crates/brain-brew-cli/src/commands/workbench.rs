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
    CanonicalDeck, CardTemplate, Note, NoteType, Overlay, OverlayKind, StableId,
    StaleTranslationRecord, TranslationCoverageCategory, TranslationCoverageEntry,
    TranslationDictionary,
};
use brain_brew_formats::canonical_yaml;
use brain_brew_formats::manifest::{
    self, BuildTarget, FederatedDeckManifest, LanguageManifestEntry, OverlayManifestEntry,
    TargetExports,
};
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
        .route(
            "/api/workbench/source-string-pivot",
            get(source_string_pivot),
        )
        .route("/api/workbench/card-pivot", get(card_pivot))
        .route(
            "/api/workbench/new-language-preview",
            get(new_language_preview),
        )
        .route("/api/workbench/new-language", post(create_new_language))
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

async fn source_string_pivot(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Query(query): Query<SourceStringPivotQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    metadata
        .source_string_pivot_json(&query)
        .map(Json)
        .map_err(workbench_api_error)
}

async fn card_pivot(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Query(query): Query<CardPivotQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    metadata
        .card_pivot_json(&query)
        .map(Json)
        .map_err(workbench_api_error)
}

async fn new_language_preview(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Query(query): Query<NewLanguagePreviewQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    metadata
        .new_language_preview_json(&query)
        .map(Json)
        .map_err(workbench_api_error)
}

async fn create_new_language(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Json(request): Json<NewLanguageRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    metadata
        .create_new_language_json(request)
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
}

impl WorkspaceMetadata {
    fn load(manifest_path: &Path, _manifest: FederatedDeckManifest) -> Self {
        Self {
            manifest_path: manifest_path.to_path_buf(),
            manifest_root: manifest_root(manifest_path),
        }
    }

    fn current_manifest(&self) -> Result<FederatedDeckManifest, String> {
        read_manifest(&self.manifest_path)
    }

    fn workspace_json(&self) -> Result<Value, String> {
        let manifest = self.current_manifest()?;
        let fingerprints = file_fingerprints(&self.manifest_path, &self.manifest_root, &manifest)?;
        Ok(json!({
            "manifest": self.manifest_path.display().to_string(),
            "manifest_root": self.manifest_root.display().to_string(),
            "languages": languages_json(&manifest),
            "target_labels": target_labels_json(&manifest),
            "targets": targets_json(&manifest),
            "translation_profile": {
                "structural_fields": manifest.translation_profile.structural_fields,
                "optional_paths": manifest.translation_profile.optional_paths,
            },
            "fingerprints": fingerprints,
        }))
    }

    fn note_pivot_json(&self, query: &NotePivotQuery) -> Result<Value, String> {
        let manifest = self.current_manifest()?;
        let context = self.selected_translation_context(
            &manifest,
            query.language.as_deref(),
            query.target.as_deref(),
            query.overlay.as_deref(),
        )?;
        Ok(note_pivot_json_from_context(
            &context,
            &manifest,
            query.filter.as_deref(),
            query.note.as_deref(),
        ))
    }

    fn source_string_pivot_json(&self, query: &SourceStringPivotQuery) -> Result<Value, String> {
        let manifest = self.current_manifest()?;
        let context = self.selected_translation_context(
            &manifest,
            query.language.as_deref(),
            query.target.as_deref(),
            query.overlay.as_deref(),
        )?;
        Ok(source_string_pivot_json_from_context(
            &context,
            &manifest,
            query.source.as_deref(),
            query.content_group.as_deref(),
            query.status.as_deref(),
        ))
    }

    fn card_pivot_json(&self, query: &CardPivotQuery) -> Result<Value, String> {
        let manifest = self.current_manifest()?;
        let context = self.selected_translation_context(
            &manifest,
            query.language.as_deref(),
            query.target.as_deref(),
            query.overlay.as_deref(),
        )?;
        Ok(card_pivot_json_from_context(
            &context,
            &manifest,
            query.card.as_deref(),
            query.filter.as_deref(),
            query.content_group.as_deref(),
        ))
    }

    fn new_language_preview_json(&self, query: &NewLanguagePreviewQuery) -> Result<Value, String> {
        let manifest = self.current_manifest()?;
        let template = if let Some(template) = &query.template {
            template.clone()
        } else {
            default_template_language(&manifest)
                .ok_or_else(|| "missing template language".to_owned())?
        };
        let code = query.code.as_deref().unwrap_or("new");
        let display_name = query.display_name.as_deref().unwrap_or(code);
        let request = default_new_language_request(&manifest, code, display_name, &template)?;
        let (updated, overlay_writes) =
            apply_new_language_request(&self.manifest_root, &manifest, &request)?;
        Ok(new_language_preview_response(
            &updated,
            &request,
            &overlay_writes,
        ))
    }

    fn create_new_language_json(&self, request: NewLanguageRequest) -> Result<Value, String> {
        let manifest = self.current_manifest()?;
        let (updated, overlay_writes) =
            apply_new_language_request(&self.manifest_root, &manifest, &request)?;
        let manifest_yaml = manifest::to_string(&updated);
        manifest::from_str(&manifest_yaml)
            .map_err(|error| format!("invalid generated manifest: {error}"))?;

        for (relative_path, overlay) in overlay_writes {
            let path = safe_new_language_relative_path(&relative_path)
                .map(|relative| self.manifest_root.join(relative))?;
            if path.exists() {
                return Err(format!(
                    "invalid new language overlay file already exists: {}",
                    path.display()
                ));
            }
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
            }
            fs::write(&path, canonical_yaml::overlay_to_string(&overlay))
                .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
        }
        fs::write(&self.manifest_path, manifest_yaml).map_err(|error| {
            format!("failed to write {}: {error}", self.manifest_path.display())
        })?;

        Ok(json!({
            "created": true,
            "language": request.code,
            "workspace": self.workspace_json()?,
        }))
    }

    fn apply_request_json(&self, request: ApplyRequest, mode: ApplyMode) -> Result<Value, String> {
        let manifest = self.current_manifest()?;
        let context = self.selected_translation_context(
            &manifest,
            request.language.as_deref(),
            request.target.as_deref(),
            request.overlay.as_deref(),
        )?;
        if request.edits.is_empty() {
            return Err("missing staged edits".to_owned());
        }

        let source_edits = request
            .edits
            .iter()
            .filter(|edit| edit.kind == WorkbenchEditKind::Source)
            .cloned()
            .collect::<Vec<_>>();
        let translation_edits = request
            .edits
            .iter()
            .filter(|edit| edit.kind == WorkbenchEditKind::Translation)
            .cloned()
            .collect::<Vec<_>>();
        let mut overlay = read_overlay_for_rewrite(&context.selection.overlay_file)?;
        let mut modified_base = context.base_deck.clone();
        let base_file = self.manifest_root.join(&manifest.base);
        let mut source_plan = apply_staged_source_edits(
            &mut modified_base,
            &mut overlay,
            &source_edits,
            &context,
            &base_file,
            &self.manifest_root,
        )?;
        let context_after_source =
            context_with_modified_base_and_overlay(&context, modified_base.clone(), &overlay)?;
        let mut changes = std::mem::take(&mut source_plan.changed_entries);
        changes.extend(apply_staged_edits_to_overlay(
            &mut overlay,
            &translation_edits,
            &context_after_source,
        )?);
        if changes.is_empty() {
            return Err("missing staged edits".to_owned());
        }

        let validation = validate_modified_base_and_overlay(&context, &modified_base, &overlay);
        if mode == ApplyMode::Write && !validation.ok {
            return Err("validation failed; preview before applying".to_owned());
        }
        if mode == ApplyMode::Write {
            write_source_apply_plan(&source_plan, &modified_base, &base_file)?;
            if source_plan.overlay_changed || !translation_edits.is_empty() {
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
        }

        let mut affected_files = source_plan.affected_files_json(&self.manifest_root);
        if source_plan.overlay_changed || !translation_edits.is_empty() {
            push_unique_affected_file(
                &mut affected_files,
                &self.manifest_root,
                &context.selection.overlay_file,
                Some(&context.selection.overlay_display_file),
            );
        }

        Ok(json!({
            "mode": mode.as_str(),
            "applied": mode == ApplyMode::Write,
            "language": context.selection.language_code,
            "target_label": context.selection.target_label,
            "target_id": context.selection.target_id,
            "overlay_label": context.selection.overlay_label,
            "overlay_id": context.selection.overlay_id,
            "affected_files": affected_files,
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
        let manifest = self.current_manifest()?;
        for target_id in manifest.targets.keys() {
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
        manifest: &FederatedDeckManifest,
        language: Option<&str>,
        target_label: Option<&str>,
        overlay_label: Option<&str>,
    ) -> Result<SelectedTranslationContext, String> {
        let selection =
            self.select_translation_target(manifest, language, target_label, overlay_label)?;
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
        manifest: &FederatedDeckManifest,
        language: Option<&str>,
        target_label: Option<&str>,
        overlay_label: Option<&str>,
    ) -> Result<WorkbenchSelection, String> {
        let (language_code, language_entry) = if let Some(language) = language {
            let entry = manifest
                .languages
                .get(language)
                .ok_or_else(|| format!("unknown language {language:?}"))?;
            (language.to_owned(), entry)
        } else {
            manifest
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
        let overlay_file = manifest
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
            structural_fields: manifest
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

#[derive(Clone, Debug, Default, Deserialize)]
struct SourceStringPivotQuery {
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    source: Option<String>,
    content_group: Option<String>,
    status: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct CardPivotQuery {
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    card: Option<String>,
    filter: Option<String>,
    content_group: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct NewLanguagePreviewQuery {
    code: Option<String>,
    display_name: Option<String>,
    template: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct NewLanguageRequest {
    code: String,
    display_name: String,
    template_language: String,
    primary_target: String,
    groups: Vec<NewLanguageGroupRequest>,
    targets: Vec<NewLanguageTargetRequest>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct NewLanguageGroupRequest {
    label: String,
    template_overlay_id: String,
    overlay_id: String,
    file: String,
    selected: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct NewLanguageTargetRequest {
    label: String,
    target_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ApplyRequest {
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    edits: Vec<StagedWorkbenchEdit>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StagedWorkbenchEdit {
    #[serde(default = "default_workbench_edit_kind")]
    kind: WorkbenchEditKind,
    path: String,
    source: String,
    value: String,
    #[serde(default = "default_edit_mode")]
    mode: EditMode,
    context_path: Option<String>,
    #[serde(default = "default_source_edit_scope")]
    scope: SourceEditScope,
    #[serde(default = "default_source_impact_action")]
    impact_action: SourceImpactAction,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum WorkbenchEditKind {
    Translation,
    Source,
}

fn default_workbench_edit_kind() -> WorkbenchEditKind {
    WorkbenchEditKind::Translation
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SourceEditScope {
    Field,
    AllOccurrences,
}

fn default_source_edit_scope() -> SourceEditScope {
    SourceEditScope::Field
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SourceImpactAction {
    StaleRecord,
    MigrateKey,
}

fn default_source_impact_action() -> SourceImpactAction {
    SourceImpactAction::StaleRecord
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

fn default_template_language(manifest: &FederatedDeckManifest) -> Option<String> {
    manifest
        .languages
        .iter()
        .find(|(_, language)| !language.source && !language.translation_overlays.is_empty())
        .map(|(code, _)| code.clone())
}

fn default_new_language_request(
    manifest: &FederatedDeckManifest,
    code: &str,
    display_name: &str,
    template_language: &str,
) -> Result<NewLanguageRequest, String> {
    let template = manifest
        .languages
        .get(template_language)
        .ok_or_else(|| format!("unknown template language {template_language:?}"))?;
    if template.source || template.translation_overlays.is_empty() {
        return Err(format!(
            "invalid template language {template_language:?}; choose a target language"
        ));
    }

    let groups = template
        .translation_overlays
        .iter()
        .map(|(label, template_overlay_id)| {
            default_new_language_group(code, label, template_overlay_id)
        })
        .collect();
    let targets = template
        .targets
        .keys()
        .map(|label| NewLanguageTargetRequest {
            label: label.clone(),
            target_id: format!("{code}-{label}"),
        })
        .collect();

    Ok(NewLanguageRequest {
        code: code.to_owned(),
        display_name: display_name.to_owned(),
        template_language: template_language.to_owned(),
        primary_target: template.primary_target.clone(),
        groups,
        targets,
    })
}

fn default_new_language_group(
    code: &str,
    label: &str,
    template_overlay_id: &str,
) -> NewLanguageGroupRequest {
    let (overlay_id, file) = if label == "base" {
        (
            format!("overlay.translation.{code}"),
            format!("overlays/languages/{code}.yaml"),
        )
    } else {
        (
            format!("overlay.translation.{label}.{code}"),
            format!("overlays/languages/{label}/{code}.yaml"),
        )
    };
    NewLanguageGroupRequest {
        label: label.to_owned(),
        template_overlay_id: template_overlay_id.to_owned(),
        overlay_id,
        file,
        selected: true,
    }
}

fn apply_new_language_request(
    manifest_root: &Path,
    manifest: &FederatedDeckManifest,
    request: &NewLanguageRequest,
) -> Result<(FederatedDeckManifest, Vec<(String, Overlay)>), String> {
    validate_new_language_code(&request.code)?;
    if request.display_name.trim().is_empty() {
        return Err("invalid new language display name: expected a non-empty value".to_owned());
    }
    if manifest.languages.contains_key(&request.code) {
        return Err(format!(
            "invalid new language code {:?}: language already exists",
            request.code
        ));
    }
    let template = manifest
        .languages
        .get(&request.template_language)
        .ok_or_else(|| format!("unknown template language {:?}", request.template_language))?;
    if template.source || template.translation_overlays.is_empty() {
        return Err(format!(
            "invalid template language {:?}; choose a target language",
            request.template_language
        ));
    }

    let template_translation_ids = template
        .translation_overlays
        .values()
        .cloned()
        .collect::<BTreeSet<_>>();
    let selected_groups = selected_new_language_groups(template, request)?;
    if selected_groups.is_empty() {
        return Err(
            "invalid new language scaffold: select at least one translation overlay group"
                .to_owned(),
        );
    }
    let selected_template_to_new = selected_groups
        .iter()
        .map(|group| (group.template_overlay_id.clone(), group.overlay_id.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut updated = manifest.clone();
    let mut overlay_writes = Vec::new();
    for group in &selected_groups {
        validate_stable_id("overlay id", &group.overlay_id)?;
        if manifest.overlays.contains_key(&group.overlay_id) {
            return Err(format!(
                "invalid new language overlay id {:?}: overlay already exists",
                group.overlay_id
            ));
        }
        let relative_path = safe_new_language_relative_path(&group.file)?;
        let absolute_path = manifest_root.join(&relative_path);
        if absolute_path.exists() {
            return Err(format!(
                "invalid new language overlay file already exists: {}",
                absolute_path.display()
            ));
        }
        let template_overlay = manifest
            .overlays
            .get(&group.template_overlay_id)
            .ok_or_else(|| format!("unknown template overlay {:?}", group.template_overlay_id))?;
        let depends_on = template_overlay
            .depends_on
            .iter()
            .filter_map(|dependency| {
                if let Some(replacement) = selected_template_to_new.get(dependency) {
                    Some(replacement.clone())
                } else if template_translation_ids.contains(dependency) {
                    None
                } else {
                    Some(dependency.clone())
                }
            })
            .collect();
        updated.overlays.insert(
            group.overlay_id.clone(),
            OverlayManifestEntry {
                file: group.file.clone(),
                kind: Some("translation".to_owned()),
                depends_on,
            },
        );
        overlay_writes.push((
            group.file.clone(),
            Overlay {
                id: StableId::new(group.overlay_id.clone()).expect("validated stable id"),
                kind: OverlayKind::Translation,
                translations: Some(TranslationDictionary::default()),
                deck_change: None,
                note_changes: BTreeMap::new(),
                note_type_changes: BTreeMap::new(),
                media_changes: BTreeMap::new(),
            },
        ));
    }

    let mut language_targets = BTreeMap::new();
    for target in &request.targets {
        validate_stable_id("target id", &target.target_id)?;
        if manifest.targets.contains_key(&target.target_id) {
            return Err(format!(
                "invalid new language target id {:?}: target already exists",
                target.target_id
            ));
        }
        let template_target_id = template.targets.get(&target.label).ok_or_else(|| {
            format!(
                "invalid new language target label {:?}: not found on template language",
                target.label
            )
        })?;
        let template_target = manifest
            .targets
            .get(template_target_id)
            .ok_or_else(|| format!("unknown template target {template_target_id:?}"))?;
        let overlays = template_target
            .overlays
            .iter()
            .filter_map(|overlay_id| {
                if let Some(replacement) = selected_template_to_new.get(overlay_id) {
                    Some(replacement.clone())
                } else if template_translation_ids.contains(overlay_id) {
                    None
                } else {
                    Some(overlay_id.clone())
                }
            })
            .collect::<Vec<_>>();
        updated.targets.insert(
            target.target_id.clone(),
            BuildTarget {
                extends: template_target.extends.clone(),
                overlays,
                translation_coverage: Default::default(),
                exports: TargetExports::default(),
            },
        );
        language_targets.insert(target.label.clone(), target.target_id.clone());
    }
    if language_targets.is_empty() {
        return Err("invalid new language scaffold: expected at least one target".to_owned());
    }
    if !language_targets.contains_key(&request.primary_target) {
        return Err(format!(
            "invalid new language primary target {:?}: not found in target labels",
            request.primary_target
        ));
    }

    let translation_overlays = selected_groups
        .iter()
        .map(|group| (group.label.clone(), group.overlay_id.clone()))
        .collect();
    updated.languages.insert(
        request.code.clone(),
        LanguageManifestEntry {
            display_name: request.display_name.clone(),
            source: false,
            translation_overlays,
            primary_target: request.primary_target.clone(),
            targets: language_targets,
        },
    );

    let manifest_yaml = manifest::to_string(&updated);
    manifest::from_str(&manifest_yaml)
        .map_err(|error| format!("invalid generated manifest: {error}"))?;
    for target in &request.targets {
        updated
            .expand_target(&target.target_id)
            .map_err(|error| format!("invalid generated target {:?}: {error}", target.target_id))?;
    }

    Ok((updated, overlay_writes))
}

fn selected_new_language_groups<'a>(
    template: &LanguageManifestEntry,
    request: &'a NewLanguageRequest,
) -> Result<Vec<&'a NewLanguageGroupRequest>, String> {
    let mut seen_labels = BTreeSet::new();
    let mut selected = Vec::new();
    for group in &request.groups {
        if !seen_labels.insert(group.label.clone()) {
            return Err(format!(
                "invalid new language overlay group {:?}: duplicate label",
                group.label
            ));
        }
        let Some(template_overlay_id) = template.translation_overlays.get(&group.label) else {
            return Err(format!(
                "invalid new language overlay group {:?}: not found on template language",
                group.label
            ));
        };
        if template_overlay_id != &group.template_overlay_id {
            return Err(format!(
                "invalid new language overlay group {:?}: template overlay changed",
                group.label
            ));
        }
        if group.selected {
            selected.push(group);
        }
    }
    Ok(selected)
}

fn new_language_preview_response(
    manifest: &FederatedDeckManifest,
    request: &NewLanguageRequest,
    overlay_writes: &[(String, Overlay)],
) -> Value {
    let overlay_files = overlay_writes
        .iter()
        .map(|(path, overlay)| {
            json!({
                "path": path,
                "contents": canonical_yaml::overlay_to_string(overlay),
            })
        })
        .collect::<Vec<_>>();
    let mut affected_files = vec![json!({ "path": "brainbrew.yaml" })];
    affected_files.extend(
        overlay_writes
            .iter()
            .map(|(path, _)| json!({ "path": path })),
    );
    json!({
        "validation": { "ok": true, "errors": Vec::<String>::new() },
        "draft": request,
        "language": {
            "code": &request.code,
            "display_name": &request.display_name,
            "template_language": &request.template_language,
            "primary_target": &request.primary_target,
        },
        "groups": &request.groups,
        "targets": &request.targets,
        "affected_files": affected_files,
        "overlay_files": overlay_files,
        "manifest_yaml": manifest::to_string(manifest),
    })
}

fn validate_new_language_code(code: &str) -> Result<(), String> {
    if code.is_empty()
        || !code
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return Err(format!(
            "invalid new language code {code:?}: expected letters, numbers, '-' or '_'"
        ));
    }
    Ok(())
}

fn validate_stable_id(kind: &str, value: &str) -> Result<(), String> {
    StableId::new(value.to_owned())
        .map(|_| ())
        .map_err(|error| format!("invalid new language {kind} {value:?}: {error}"))
}

fn safe_new_language_relative_path(path: &str) -> Result<PathBuf, String> {
    let candidate = Path::new(path);
    if path.is_empty() || path.starts_with('~') || candidate.is_absolute() {
        return Err(format!("invalid new language file path {path:?}"));
    }
    if !candidate
        .components()
        .all(|component| matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!("invalid new language file path {path:?}"));
    }
    Ok(candidate.to_path_buf())
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

fn source_string_pivot_json_from_context(
    context: &SelectedTranslationContext,
    manifest: &FederatedDeckManifest,
    selected_source: Option<&str>,
    content_group_filter: Option<&str>,
    status_filter: Option<&str>,
) -> Value {
    let content_group_filter = content_group_filter.filter(|filter| *filter != "all");
    let status_filter = status_filter.filter(|filter| *filter != "all");
    let rows = source_string_rows(context)
        .into_iter()
        .filter(|row| !row.structural && !row.source.is_empty())
        .filter(|row| {
            content_group_filter.is_none_or(|filter| {
                content_group_badges_for_note(&context.source_deck, &row.note_id)
                    .iter()
                    .any(|badge| badge == filter)
            })
        })
        .collect::<Vec<_>>();
    let source_counts = rows
        .iter()
        .fold(BTreeMap::<String, usize>::new(), |mut counts, row| {
            *counts.entry(row.source.clone()).or_insert(0) += 1;
            counts
        });
    let grouped = source_string_groups(&context.source_deck, &rows);
    let filtered_groups = grouped
        .into_iter()
        .filter(|group| status_filter.is_none_or(|filter| filter == group.status))
        .collect::<Vec<_>>();
    let selected_source = selected_source
        .filter(|source| filtered_groups.iter().any(|group| group.source == *source))
        .map(str::to_owned)
        .or_else(|| filtered_groups.first().map(|group| group.source.clone()));
    let strings = filtered_groups
        .iter()
        .map(|group| {
            json!({
                "source": group.source,
                "status": group.status,
                "main_completion_status": group.status,
                "occurrence_count": group.occurrence_count,
                "complete_count": group.complete_count,
                "missing_count": group.missing_count,
                "stale_count": group.stale_count,
                "target_preview": group.target_preview,
                "content_group_badges": group.content_group_badges,
                "direct_recommended": true,
                "direct_applies_to": source_counts.get(&group.source).copied().unwrap_or(group.occurrence_count),
                "selected": selected_source.as_deref() == Some(group.source.as_str()),
            })
        })
        .collect::<Vec<_>>();
    let occurrences = selected_source
        .as_deref()
        .map(|source| {
            rows.iter()
                .filter(|row| row.source == source)
                .map(|row| source_string_occurrence_json(context, row))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let content_groups = rows
        .iter()
        .flat_map(|row| content_group_badges_for_note(&context.source_deck, &row.note_id))
        .collect::<BTreeSet<_>>();

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
        "selection_options": selection_options_json(context, manifest),
        "filters": {
            "content_group": content_group_filter.unwrap_or("all"),
            "content_groups": content_groups,
            "status": status_filter.unwrap_or("all"),
            "statuses": ["all", "missing", "stale", "complete"],
        },
        "strings": strings,
        "selected_source": selected_source,
        "occurrences": occurrences,
    })
}

fn card_pivot_json_from_context(
    context: &SelectedTranslationContext,
    manifest: &FederatedDeckManifest,
    selected_card: Option<&str>,
    filter: Option<&str>,
    content_group_filter: Option<&str>,
) -> Value {
    let content_group_filter = content_group_filter.filter(|filter| *filter != "all");
    let rows = main_field_rows(
        &context.source_deck,
        &context.selection,
        &context.report.entries,
    );
    let cards = produced_card_rows(context, &rows)
        .into_iter()
        .filter(|card| card_matches_filter(card, filter))
        .filter(|card| {
            content_group_filter.is_none_or(|filter| {
                card.content_group_badges
                    .iter()
                    .any(|badge| badge == filter)
            })
        })
        .collect::<Vec<_>>();
    let selected_card_id = selected_card
        .filter(|card_id| cards.iter().any(|card| card.card_id == *card_id))
        .map(str::to_owned)
        .or_else(|| cards.first().map(|card| card.card_id.clone()));
    let selected = selected_card_id
        .as_deref()
        .and_then(|card_id| cards.iter().find(|card| card.card_id == card_id))
        .map(|card| card_detail_json(context, card));
    let content_groups = cards
        .iter()
        .flat_map(|card| card.content_group_badges.iter().cloned())
        .collect::<BTreeSet<_>>();
    let total = cards.len();
    let missing = cards.iter().filter(|card| card.status == "missing").count();
    let stale = cards.iter().filter(|card| card.status == "stale").count();
    let complete = total.saturating_sub(missing + stale);

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
        "selection_options": selection_options_json(context, manifest),
        "filters": {
            "active": filter.unwrap_or("all"),
            "available": ["all", "missing", "stale", "needs_work"],
            "content_group": content_group_filter.unwrap_or("all"),
            "content_groups": content_groups,
        },
        "progress": {
            "total": total,
            "complete": complete,
            "missing": missing,
            "stale": stale,
        },
        "cards": cards.iter().map(card_summary_json).collect::<Vec<_>>(),
        "selected_card_id": selected_card_id,
        "selected_card": selected,
    })
}

#[derive(Clone, Debug)]
struct ProducedCardRow {
    card_id: String,
    note_id: StableId,
    note_type_id: StableId,
    template_id: StableId,
    template_name: String,
    title: String,
    status: String,
    field_rows: Vec<MainFieldRow>,
    content_group_badges: BTreeSet<String>,
}

fn produced_card_rows(
    context: &SelectedTranslationContext,
    rows: &[MainFieldRow],
) -> Vec<ProducedCardRow> {
    let rows_by_note_field = rows
        .iter()
        .map(|row| ((row.note_id.clone(), row.field_id.clone()), row.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut cards = Vec::new();
    for (note_id, note) in &context.source_deck.notes {
        let Some(note_type) = context.source_deck.note_types.get(&note.note_type_id) else {
            continue;
        };
        for template in &note_type.card_templates {
            let used_fields = card_used_field_ids(note_type, template);
            let field_rows = used_fields
                .iter()
                .filter_map(|field_id| {
                    rows_by_note_field
                        .get(&(note_id.clone(), field_id.clone()))
                        .cloned()
                })
                .collect::<Vec<_>>();
            let status = card_status(&field_rows);
            let card_id = format!("{note_id}::{}", template.id);
            cards.push(ProducedCardRow {
                card_id,
                note_id: note_id.clone(),
                note_type_id: note.note_type_id.clone(),
                template_id: template.id.clone(),
                template_name: template.name.clone(),
                title: note_title(Some(note)),
                status: status.to_owned(),
                field_rows,
                content_group_badges: content_group_badges_for_note(&context.source_deck, note_id),
            });
        }
    }
    cards
}

fn card_used_field_ids(note_type: &NoteType, template: &CardTemplate) -> BTreeSet<StableId> {
    let template_text = format!("{}\n{}", template.question_format, template.answer_format);
    let mut fields = BTreeSet::new();
    for field in &note_type.fields {
        let name = &field.name;
        let markers = [
            format!("{{{{{name}}}}}"),
            format!("{{{{#{name}}}}}"),
            format!("{{{{^{name}}}}}"),
            format!("{{{{type:{name}}}}}"),
        ];
        if markers.iter().any(|marker| template_text.contains(marker)) {
            fields.insert(field.id.clone());
        }
    }
    if fields.is_empty() {
        fields.extend(note_type.fields.iter().map(|field| field.id.clone()));
    }
    fields
}

fn card_status(rows: &[MainFieldRow]) -> &'static str {
    if rows.iter().any(|row| {
        !row.structural && row.category == TranslationCoverageCategory::UntranslatedFallback
    }) {
        "missing"
    } else if rows.iter().any(|row| is_stale_category(row.category)) {
        "stale"
    } else {
        "complete"
    }
}

fn card_matches_filter(card: &ProducedCardRow, filter: Option<&str>) -> bool {
    match filter.unwrap_or("all") {
        "missing" => card.status == "missing",
        "stale" => card.status == "stale",
        "needs_work" | "needs-work" => matches!(card.status.as_str(), "missing" | "stale"),
        _ => true,
    }
}

fn card_summary_json(card: &ProducedCardRow) -> Value {
    json!({
        "card_id": card.card_id,
        "note_id": card.note_id.to_string(),
        "note_type_id": card.note_type_id.to_string(),
        "template_id": card.template_id.to_string(),
        "template_name": card.template_name,
        "title": card.title,
        "status": card.status,
        "field_count": card.field_rows.len(),
        "content_group_badges": card.content_group_badges,
    })
}

fn card_detail_json(context: &SelectedTranslationContext, card: &ProducedCardRow) -> Value {
    let source_note = context.source_deck.notes.get(&card.note_id);
    let target_note = context.target_deck.notes.get(&card.note_id).or(source_note);
    let source_note_type = context.source_deck.note_types.get(&card.note_type_id);
    let target_note_type = target_note
        .and_then(|note| context.target_deck.note_types.get(&note.note_type_id))
        .or(source_note_type);
    let target_fields = target_note.map(|note| &note.fields);
    let fields = card
        .field_rows
        .iter()
        .map(|row| {
            let target = target_fields
                .and_then(|fields| fields.get(&row.field_id))
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
                "structural": row.structural,
                "editable": !row.structural,
                "source_editable": true,
                "context_path": contextual_path_for_row(row, &context.report.entries),
                "controls": ["direct", "contextual", "no_change"],
            })
        })
        .collect::<Vec<_>>();
    json!({
        "card_id": card.card_id,
        "note_id": card.note_id.to_string(),
        "note_type_id": card.note_type_id.to_string(),
        "template_id": card.template_id.to_string(),
        "template_name": card.template_name,
        "title": card.title,
        "status": card.status,
        "content_group_badges": card.content_group_badges,
        "fields": fields,
        "source_preview": source_note.and_then(|note| source_note_type.map(|note_type| render_single_note_card(&context.source_deck, note, note_type, &card.template_id))),
        "target_preview": target_note.and_then(|note| target_note_type.map(|note_type| render_single_note_card(&context.target_deck, note, note_type, &card.template_id))),
    })
}

fn render_single_note_card(
    deck: &CanonicalDeck,
    note: &Note,
    note_type: &NoteType,
    template_id: &StableId,
) -> Value {
    let rendered_deck = deck.render_variables().unwrap_or_else(|_| deck.clone());
    let rendered_note = rendered_deck.notes.get(&note.id).unwrap_or(note);
    let rendered_note_type = rendered_deck
        .note_types
        .get(&note_type.id)
        .unwrap_or(note_type);
    let cards = rendered_note_type
        .card_templates
        .iter()
        .filter(|template| &template.id == template_id)
        .map(|template| render_card(rendered_note, rendered_note_type, template))
        .collect::<Vec<_>>();
    json!({
        "styling": rendered_note_type.styling,
        "cards": cards,
    })
}

fn source_string_rows(context: &SelectedTranslationContext) -> Vec<MainFieldRow> {
    let structural_fields = structural_field_set(&context.selection, &context.source_deck);
    context
        .source_deck
        .translation_context(&context.report)
        .units
        .into_iter()
        .filter_map(|unit| {
            let note_id = unit.note_id?;
            let note_type_id = unit.note_type_id?;
            let field_id = unit.field_id?;
            let field_name = unit.field_name.unwrap_or_else(|| field_id.to_string());
            Some(MainFieldRow {
                note_id,
                note_type_id,
                structural: structural_fields.contains(field_id.as_str()),
                field_id,
                field_name,
                path: unit.path,
                source: unit.source.clone(),
                category: unit.category,
                translated: unit.translated.unwrap_or(unit.source),
            })
        })
        .collect()
}

#[derive(Clone, Debug)]
struct SourceStringGroup {
    source: String,
    status: String,
    occurrence_count: usize,
    complete_count: usize,
    missing_count: usize,
    stale_count: usize,
    target_preview: Option<String>,
    content_group_badges: BTreeSet<String>,
}

fn source_string_groups(
    source_deck: &CanonicalDeck,
    rows: &[MainFieldRow],
) -> Vec<SourceStringGroup> {
    let mut groups = BTreeMap::<String, Vec<&MainFieldRow>>::new();
    for row in rows {
        groups.entry(row.source.clone()).or_default().push(row);
    }
    groups
        .into_iter()
        .map(|(source, rows)| {
            let occurrence_count = rows.len();
            let complete_count = rows
                .iter()
                .filter(|row| is_complete_translation_category(row.category))
                .count();
            let missing_count = rows
                .iter()
                .filter(|row| row.category == TranslationCoverageCategory::UntranslatedFallback)
                .count();
            let stale_count = rows
                .iter()
                .filter(|row| is_stale_category(row.category))
                .count();
            let status = if missing_count > 0 {
                "missing"
            } else if stale_count > 0 {
                "stale"
            } else {
                "complete"
            }
            .to_owned();
            let target_preview = rows
                .iter()
                .copied()
                .find(|row| row.translated != row.source)
                .or_else(|| rows.first().copied())
                .map(|row| row.translated.clone());
            let content_group_badges = rows
                .iter()
                .flat_map(|row| content_group_badges_for_note(source_deck, &row.note_id))
                .collect::<BTreeSet<_>>();
            SourceStringGroup {
                source,
                status,
                occurrence_count,
                complete_count,
                missing_count,
                stale_count,
                target_preview,
                content_group_badges,
            }
        })
        .collect()
}

fn source_string_occurrence_json(
    context: &SelectedTranslationContext,
    row: &MainFieldRow,
) -> Value {
    let note = context.source_deck.notes.get(&row.note_id);
    let target_note = context.target_deck.notes.get(&row.note_id).or(note);
    let source_note_type = context.source_deck.note_types.get(&row.note_type_id);
    let target_note_type = target_note
        .and_then(|note| context.target_deck.note_types.get(&note.note_type_id))
        .or(source_note_type);
    json!({
        "path": row.path,
        "source": row.source,
        "target": if row.path.contains(".message.") {
            row.translated.clone()
        } else {
            target_note
                .and_then(|note| note.fields.get(&row.field_id))
                .cloned()
                .unwrap_or_else(|| row.translated.clone())
        },
        "status": row.category.as_str(),
        "note_id": row.note_id.to_string(),
        "note_title": note_title(note),
        "field_id": row.field_id.to_string(),
        "field_name": row.field_name,
        "friendly_context": format!("{} · {}", note_title(note), row.field_name),
        "context_path": contextual_path_for_row(row, &context.report.entries),
        "content_group_badges": content_group_badges_for_note(&context.source_deck, &row.note_id),
        "direct_recommended": true,
        "controls": ["direct", "contextual", "no_change"],
        "source_preview": note.and_then(|note| source_note_type.map(|note_type| render_note_cards(&context.source_deck, note, note_type))),
        "target_preview": target_note.and_then(|note| target_note_type.map(|note_type| render_note_cards(&context.target_deck, note, note_type))),
    })
}

fn note_title(note: Option<&Note>) -> String {
    note.and_then(|note| note.fields.values().next().cloned())
        .unwrap_or_else(|| "unknown note".to_owned())
}

fn content_group_badges_for_note(
    source_deck: &CanonicalDeck,
    note_id: &StableId,
) -> BTreeSet<String> {
    let mut badges = BTreeSet::new();
    if let Some(note) = source_deck.notes.get(note_id) {
        badges.insert(note.note_type_id.to_string());
        badges.extend(note.tags.iter().cloned());
    }
    badges
}

fn is_complete_translation_category(category: TranslationCoverageCategory) -> bool {
    matches!(
        category,
        TranslationCoverageCategory::DirectTranslation
            | TranslationCoverageCategory::ContextualOverride
            | TranslationCoverageCategory::NoChange
            | TranslationCoverageCategory::StaleTranslationRecord
    )
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
                    | TranslationCoverageCategory::StaleTranslationRecord
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
                    "source_editable": true,
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
                "old_source": entry.old_source,
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
            | TranslationCoverageCategory::StaleTranslationRecord
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

#[derive(Default)]
struct SourceApplyPlan {
    changed_entries: Vec<Value>,
    overlay_changed: bool,
    deck_file_changed: bool,
    deck_yaml_output: Option<String>,
    include_writes: BTreeMap<PathBuf, String>,
    affected_files: BTreeMap<PathBuf, String>,
}

impl SourceApplyPlan {
    fn affected_files_json(&self, _root: &Path) -> Vec<Value> {
        self.affected_files
            .iter()
            .map(|(absolute_path, display_path)| {
                json!({
                    "path": display_path,
                    "absolute_path": absolute_path.display().to_string(),
                })
            })
            .collect()
    }
}

fn apply_staged_source_edits(
    modified_base: &mut CanonicalDeck,
    overlay: &mut Overlay,
    edits: &[StagedWorkbenchEdit],
    context: &SelectedTranslationContext,
    base_file: &Path,
    manifest_root: &Path,
) -> Result<SourceApplyPlan, String> {
    let mut plan = SourceApplyPlan::default();
    if edits.is_empty() {
        return Ok(plan);
    }

    let raw_deck_yaml = fs::read_to_string(base_file)
        .map_err(|error| format!("{}: {error}", base_file.display()))?;
    let raw_has_includes = raw_deck_yaml.contains("!include");
    let mut raw_deck_value = if raw_has_includes {
        Some(
            serde_yaml::from_str::<serde_yaml::Value>(&raw_deck_yaml).map_err(|error| {
                format!(
                    "failed to parse {} while preserving includes: {error}",
                    base_file.display()
                )
            })?,
        )
    } else {
        None
    };

    let rows = main_field_rows(
        &context.source_deck,
        &context.selection,
        &context.report.entries,
    );
    let row_by_path = rows
        .iter()
        .map(|row| (row.path.clone(), row.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut changed_paths = BTreeSet::new();

    for edit in edits {
        if !edit.path.starts_with("notes.") || !edit.path.contains(".fields.") {
            return Err(format!(
                "invalid source note-field edit path {:?}",
                edit.path
            ));
        }
        if edit.source.is_empty() {
            return Err(format!("invalid empty source for {}", edit.path));
        }
        let Some(anchor_row) = row_by_path.get(&edit.path) else {
            return Err(format!(
                "invalid source edit path {:?}; choose an editable source note field",
                edit.path
            ));
        };
        if anchor_row.source != edit.source {
            return Err(format!(
                "invalid source {:?} for {}; expected {:?}",
                edit.source, edit.path, anchor_row.source
            ));
        }
        let target_rows = match edit.scope {
            SourceEditScope::Field => vec![anchor_row.clone()],
            SourceEditScope::AllOccurrences => rows
                .iter()
                .filter(|row| row.source == edit.source)
                .cloned()
                .collect::<Vec<_>>(),
        };
        let target_path_set = target_rows
            .iter()
            .map(|row| row.path.clone())
            .collect::<BTreeSet<_>>();
        let old_source_remains = rows
            .iter()
            .any(|row| row.source == edit.source && !target_path_set.contains(row.path.as_str()));

        for row in &target_rows {
            set_deck_note_field(modified_base, &row.path, &edit.source, &edit.value)?;
            changed_paths.insert(row.path.clone());
            if let Some(raw_deck_value) = raw_deck_value.as_mut() {
                match deck_field_include_path(raw_deck_value, &row.path)? {
                    Some(include_path) => {
                        let resolved =
                            resolve_workbench_include_path(manifest_root, &include_path)?;
                        plan.include_writes.insert(resolved, edit.value.clone());
                    }
                    None => {
                        set_deck_field_yaml_scalar(
                            raw_deck_value,
                            &row.path,
                            &edit.source,
                            &edit.value,
                        )?;
                        plan.deck_file_changed = true;
                    }
                }
            } else {
                plan.deck_file_changed = true;
            }
            plan.changed_entries.push(json!({
                "mode": "source",
                "path": row.path,
                "source": edit.source,
                "old": edit.source,
                "new": edit.value,
                "scope": edit.scope,
            }));
        }

        plan.overlay_changed |= apply_source_translation_impact(
            overlay,
            edit,
            &target_rows,
            old_source_remains,
            context,
            &mut plan.changed_entries,
        )?;
    }

    if plan.deck_file_changed {
        plan.affected_files.insert(
            base_file.to_path_buf(),
            workspace_path(manifest_root, base_file),
        );
    }
    for path in plan.include_writes.keys() {
        plan.affected_files
            .insert(path.clone(), workspace_path(manifest_root, path));
    }
    if let Some(raw_deck_value) = raw_deck_value
        && plan.deck_file_changed
    {
        plan.deck_yaml_output = Some(serde_yaml::to_string(&raw_deck_value).map_err(|error| {
            format!(
                "failed to serialize {} while preserving includes: {error}",
                base_file.display()
            )
        })?);
    }
    Ok(plan)
}

fn apply_source_translation_impact(
    overlay: &mut Overlay,
    edit: &StagedWorkbenchEdit,
    target_rows: &[MainFieldRow],
    old_source_remains: bool,
    context: &SelectedTranslationContext,
    changed_entries: &mut Vec<Value>,
) -> Result<bool, String> {
    let mut changed = false;
    if target_rows.is_empty() {
        return Ok(false);
    }
    let use_global_impact = edit.scope == SourceEditScope::AllOccurrences
        && !old_source_remains
        && target_rows.iter().all(|row| {
            matches!(
                row.category,
                TranslationCoverageCategory::DirectTranslation
                    | TranslationCoverageCategory::NoChange
                    | TranslationCoverageCategory::StaleTranslationRecord
            )
        })
        && target_rows.iter().all(|row| {
            row.category != TranslationCoverageCategory::StaleTranslationRecord
                || context
                    .report
                    .entries
                    .iter()
                    .find(|entry| entry.path == row.path && entry.source == row.source)
                    .and_then(|entry| entry.context.as_deref())
                    .is_none()
        })
        && target_rows
            .iter()
            .map(target_text_for_source_impact)
            .collect::<Option<BTreeSet<_>>>()
            .is_some_and(|targets| targets.len() <= 1);
    let impact_rows = if use_global_impact {
        vec![target_rows[0].clone()]
    } else {
        target_rows.to_vec()
    };
    let translations = overlay.translations.get_or_insert_with(Default::default);
    for row in impact_rows {
        let Some(target) = target_text_for_source_impact(&row) else {
            continue;
        };
        let context_path = if use_global_impact {
            None
        } else {
            Some(contextual_path_for_row(&row, &context.report.entries))
        };
        match edit.impact_action {
            SourceImpactAction::StaleRecord => {
                if context_path.is_none() {
                    translations.direct.remove(&edit.source);
                    translations.no_change.remove(&edit.source);
                    remove_contextual_source_everywhere(translations, &edit.source);
                } else {
                    remove_contextual_source_for_path(translations, &row.path, &edit.source);
                }
                upsert_stale_record(
                    translations,
                    StaleTranslationRecord {
                        old_source: edit.source.clone(),
                        new_source: edit.value.clone(),
                        target: target.clone(),
                        context: context_path.clone(),
                    },
                );
                changed_entries.push(json!({
                    "mode": "stale_record",
                    "path": row.path,
                    "old_source": edit.source,
                    "new_source": edit.value,
                    "target": target,
                    "context": context_path,
                    "impact_action": "stale_record",
                    "available_impact_actions": ["stale_record", "migrate_key"],
                }));
                changed = true;
            }
            SourceImpactAction::MigrateKey => {
                if let Some(context_path) = &context_path {
                    remove_contextual_source_for_path(translations, &row.path, &edit.source);
                    translations
                        .contextual
                        .entry(context_path.clone())
                        .or_default()
                        .insert(edit.value.clone(), target.clone());
                    remove_stale_records_for_path_source(translations, &row.path, &edit.value);
                } else {
                    translations.direct.remove(&edit.source);
                    translations.no_change.remove(&edit.source);
                    remove_contextual_source_everywhere(translations, &edit.source);
                    translations
                        .direct
                        .insert(edit.value.clone(), target.clone());
                    remove_stale_records_for_path_source(translations, &row.path, &edit.value);
                }
                changed_entries.push(json!({
                    "mode": "migrate_key",
                    "path": row.path,
                    "old_source": edit.source,
                    "new_source": edit.value,
                    "target": target,
                    "context": context_path,
                    "impact_action": "migrate_key",
                    "available_impact_actions": ["stale_record", "migrate_key"],
                }));
                changed = true;
            }
        }
    }
    Ok(changed)
}

fn target_text_for_source_impact(row: &MainFieldRow) -> Option<String> {
    match row.category {
        TranslationCoverageCategory::DirectTranslation
        | TranslationCoverageCategory::ContextualOverride
        | TranslationCoverageCategory::NoChange
        | TranslationCoverageCategory::StaleTranslationRecord => Some(row.translated.clone()),
        _ => None,
    }
}

fn upsert_stale_record(translations: &mut TranslationDictionary, record: StaleTranslationRecord) {
    translations.stale_records.retain(|existing| {
        !(existing.old_source == record.old_source
            && existing.new_source == record.new_source
            && existing.context == record.context)
    });
    translations.stale_records.push(record);
}

fn remove_contextual_source_everywhere(translations: &mut TranslationDictionary, source: &str) {
    let contexts = translations
        .contextual
        .iter()
        .filter(|(_, replacements)| replacements.contains_key(source))
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

fn set_deck_note_field(
    deck: &mut CanonicalDeck,
    path: &str,
    expected_source: &str,
    value: &str,
) -> Result<(), String> {
    let (note_id, field_id) = note_field_path(path)?;
    let note_id = StableId::new(note_id).map_err(|error| error.to_string())?;
    let field_id = StableId::new(field_id).map_err(|error| error.to_string())?;
    let note = deck
        .notes
        .get_mut(&note_id)
        .ok_or_else(|| format!("source edit path {path:?} is not in the canonical deck file"))?;
    let current = note.fields.get(&field_id).cloned().unwrap_or_default();
    if current != expected_source {
        return Err(format!(
            "invalid source {:?} for {}; expected canonical deck value {:?}",
            expected_source, path, current
        ));
    }
    note.fields.insert(field_id, value.to_owned());
    Ok(())
}

fn note_field_path(path: &str) -> Result<(&str, &str), String> {
    let Some(rest) = path.strip_prefix("notes.") else {
        return Err(format!("invalid note-field path {path:?}"));
    };
    let Some((note_id, field_id)) = rest.split_once(".fields.") else {
        return Err(format!("invalid note-field path {path:?}"));
    };
    if note_id.is_empty() || field_id.is_empty() {
        return Err(format!("invalid note-field path {path:?}"));
    }
    Ok((note_id, field_id))
}

fn deck_field_include_path(
    value: &serde_yaml::Value,
    path: &str,
) -> Result<Option<String>, String> {
    let field = deck_field_yaml_value(value, path)?;
    match field {
        serde_yaml::Value::Tagged(tagged) if tagged.tag == "include" => match &tagged.value {
            serde_yaml::Value::String(path) => Ok(Some(path.clone())),
            _ => Err(format!("invalid !include at {path}: path must be a string")),
        },
        _ => Ok(None),
    }
}

fn set_deck_field_yaml_scalar(
    value: &mut serde_yaml::Value,
    path: &str,
    expected_source: &str,
    new_value: &str,
) -> Result<(), String> {
    let field = deck_field_yaml_value_mut(value, path)?;
    match field {
        serde_yaml::Value::String(current) if current == expected_source => {
            *current = new_value.to_owned();
            Ok(())
        }
        serde_yaml::Value::String(current) => Err(format!(
            "invalid source {:?} for {}; expected YAML value {:?}",
            expected_source, path, current
        )),
        serde_yaml::Value::Tagged(tagged) if tagged.tag == "include" => Ok(()),
        _ => Err(format!(
            "source edit path {path:?} is not a scalar note field in deck YAML"
        )),
    }
}

fn deck_field_yaml_value<'a>(
    value: &'a serde_yaml::Value,
    path: &str,
) -> Result<&'a serde_yaml::Value, String> {
    let (note_id, field_id) = note_field_path(path)?;
    yaml_mapping_get(value, "notes")
        .and_then(|notes| yaml_mapping_get(notes, note_id))
        .and_then(|note| yaml_mapping_get(note, "fields"))
        .and_then(|fields| yaml_mapping_get(fields, field_id))
        .ok_or_else(|| format!("source edit path {path:?} is not present in deck YAML"))
}

fn deck_field_yaml_value_mut<'a>(
    value: &'a mut serde_yaml::Value,
    path: &str,
) -> Result<&'a mut serde_yaml::Value, String> {
    let (note_id, field_id) = note_field_path(path)?;
    yaml_mapping_get_mut(value, "notes")
        .and_then(|notes| yaml_mapping_get_mut(notes, note_id))
        .and_then(|note| yaml_mapping_get_mut(note, "fields"))
        .and_then(|fields| yaml_mapping_get_mut(fields, field_id))
        .ok_or_else(|| format!("source edit path {path:?} is not present in deck YAML"))
}

fn yaml_mapping_get<'a>(value: &'a serde_yaml::Value, key: &str) -> Option<&'a serde_yaml::Value> {
    let serde_yaml::Value::Mapping(mapping) = value else {
        return None;
    };
    mapping.get(serde_yaml::Value::String(key.to_owned()))
}

fn yaml_mapping_get_mut<'a>(
    value: &'a mut serde_yaml::Value,
    key: &str,
) -> Option<&'a mut serde_yaml::Value> {
    let serde_yaml::Value::Mapping(mapping) = value else {
        return None;
    };
    mapping.get_mut(serde_yaml::Value::String(key.to_owned()))
}

fn resolve_workbench_include_path(root: &Path, include_path: &str) -> Result<PathBuf, String> {
    let requested = Path::new(include_path);
    if requested.is_absolute() || include_path.contains("..") {
        return Err(format!(
            "source edit include path {include_path:?} is not a safe package-root-relative path"
        ));
    }
    Ok(root.join(requested))
}

fn write_source_apply_plan(
    plan: &SourceApplyPlan,
    modified_base: &CanonicalDeck,
    base_file: &Path,
) -> Result<(), String> {
    if plan.deck_file_changed {
        let output = match &plan.deck_yaml_output {
            Some(output) => output.clone(),
            None => canonical_yaml::to_string(modified_base).map_err(|error| error.to_string())?,
        };
        fs::write(base_file, output).map_err(|error| {
            format!(
                "failed to write source deck {}: {error}",
                base_file.display()
            )
        })?;
    }
    for (path, value) in &plan.include_writes {
        fs::write(path, value).map_err(|error| {
            format!(
                "failed to write included source {}: {error}",
                path.display()
            )
        })?;
    }
    Ok(())
}

fn push_unique_affected_file(
    files: &mut Vec<Value>,
    root: &Path,
    absolute_path: &Path,
    display_path: Option<&str>,
) {
    let absolute = absolute_path.display().to_string();
    if files
        .iter()
        .any(|file| file["absolute_path"].as_str() == Some(absolute.as_str()))
    {
        return;
    }
    files.push(json!({
        "path": display_path
            .map(str::to_owned)
            .unwrap_or_else(|| workspace_path(root, absolute_path)),
        "absolute_path": absolute,
    }));
}

fn editable_sources_by_path(context: &SelectedTranslationContext) -> BTreeMap<String, String> {
    let mut sources = main_field_rows(
        &context.source_deck,
        &context.selection,
        &context.report.entries,
    )
    .into_iter()
    .filter(|row| !row.structural)
    .map(|row| (row.path, row.source))
    .collect::<BTreeMap<_, _>>();
    for row in source_string_rows(context)
        .into_iter()
        .filter(|row| !row.structural)
    {
        sources.entry(row.path).or_insert(row.source);
    }
    sources
}

fn contextual_path_for_edit(
    context: &SelectedTranslationContext,
    edit: &StagedWorkbenchEdit,
) -> Result<String, String> {
    let rows = main_field_rows(
        &context.source_deck,
        &context.selection,
        &context.report.entries,
    );
    let source_rows = if rows
        .iter()
        .any(|row| row.path == edit.path && row.source == edit.source && !row.structural)
    {
        rows
    } else {
        source_string_rows(context)
    };
    let row = source_rows
        .iter()
        .find(|row| row.path == edit.path && row.source == edit.source && !row.structural)
        .ok_or_else(|| format!("invalid contextual edit path {:?}", edit.path))?;
    if let Some(context_path) = &edit.context_path {
        if context_path == &row.path
            || row
                .path
                .strip_prefix(context_path)
                .is_some_and(|suffix| suffix.starts_with('.'))
        {
            return Ok(context_path.clone());
        }
        return Err(format!(
            "invalid contextual edit context {:?} for {}",
            context_path, edit.path
        ));
    }
    Ok(contextual_path_for_row(row, &context.report.entries))
}

fn apply_staged_edits_to_overlay(
    overlay: &mut Overlay,
    edits: &[StagedWorkbenchEdit],
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
                remove_stale_records_for_path_source(translations, &edit.path, &edit.source);
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
                remove_stale_records_for_path_source(translations, &edit.path, &edit.source);
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

fn remove_stale_records_for_path_source(
    translations: &mut TranslationDictionary,
    path: &str,
    source: &str,
) {
    translations.stale_records.retain(|record| {
        !(record.new_source == source
            && record.context.as_deref().is_none_or(|context| {
                path == context
                    || path
                        .strip_prefix(context)
                        .is_some_and(|suffix| suffix.starts_with('.'))
            }))
    });
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

fn validate_modified_base_and_overlay(
    context: &SelectedTranslationContext,
    modified_base: &CanonicalDeck,
    modified_overlay: &Overlay,
) -> ApplyValidation {
    match context_with_modified_base_and_overlay(context, modified_base.clone(), modified_overlay) {
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

fn context_with_modified_base_and_overlay(
    context: &SelectedTranslationContext,
    modified_base: CanonicalDeck,
    modified_overlay: &Overlay,
) -> Result<SelectedTranslationContext, String> {
    let mut current = modified_base.clone();
    let mut selected_source_deck = None;
    let mut selected_report = None;
    for (planned, overlay) in &context.plan_overlays {
        let active_overlay = if planned.id == context.selection.overlay_id {
            modified_overlay
        } else {
            overlay
        };
        if planned.id == context.selection.overlay_id {
            selected_source_deck = Some(current.clone());
            selected_report = current.translation_coverage(active_overlay);
        }
        current = if active_overlay.translations.is_some() {
            compose_lenient_translation_overlay(&current, active_overlay)?
        } else {
            current
                .compose(std::slice::from_ref(active_overlay))
                .map_err(|error| format!("failed to compose overlay {}: {error}", planned.id))?
        };
    }
    let Some(source_deck) = selected_source_deck else {
        return Err(format!(
            "target {} does not include translation overlay {}",
            context.selection.target_id, context.selection.overlay_id
        ));
    };
    let Some(report) = selected_report else {
        return Err(format!(
            "overlay {} is not a translation overlay",
            context.selection.overlay_id
        ));
    };
    Ok(SelectedTranslationContext {
        selection: context.selection.clone(),
        base_deck: modified_base,
        plan_overlays: context.plan_overlays.clone(),
        source_deck,
        target_deck: current,
        report,
    })
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
