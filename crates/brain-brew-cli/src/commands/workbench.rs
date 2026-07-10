use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;

use axum::body::Body;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use brain_brew_core::{
    CanonicalDeck, CardTemplate, ContentKind, ContentValidationReport, DiagnosticCategory,
    DomainDiagnostic, FieldGraphReport, FieldValue, Note, NoteType, Overlay, OverlayKind, StableId,
    TranslationCoverageCategory, TranslationCoverageEntry, TranslationDictionary,
    validate_deck_content,
};
use brain_brew_formats::canonical_source_document::CanonicalScalarTarget;
use brain_brew_formats::manifest::{
    self, BuildTarget, FederatedDeckManifest, LanguageManifestEntry, MetadataCategory,
    OverlayManifestEntry, TargetExports,
};
use brain_brew_formats::overlay_source_document::{
    OverlaySourceDocument, SourceTranslationImpact, TranslationDecision,
};
use brain_brew_formats::safe_relative_path::SafeRelativePath;
use brain_brew_formats::source_document::{EditLocation, SourceDocumentEmission};
use brain_brew_formats::{canonical_yaml, media};
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::task;
use tower_http::services::{ServeDir, ServeFile};

use crate::commands::translation_overlay::sanitize_lenient_translation_overlay;
use crate::help;
use crate::io::{canonical_source_document, manifest_root, overlay_source_document, read_manifest};
use crate::media_ownership::MediaRootSelections;
use crate::path_authorization::PathAuthorizer;
use crate::planner::{
    ManifestRegistry, MediaDeclarationProvenance, PlannedOverlay, RegistrySourceKind,
    SourceProvenance as PlannedSourceProvenance, plan_manifest_target,
};
use crate::workspace_mutation::{PlannedWorkspaceFile, commit_workspace_files, recover_workspace};

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
    media_roots: Vec<String>,
    enable_write: bool,
}

fn parse_serve_args(args: &[String]) -> Result<ServeArgs, String> {
    let mut parsed = ServeArgs {
        manifest_path: PathBuf::from("brainbrew.yaml"),
        port: 0,
        open_browser: true,
        dev_assets: None,
        media_roots: Vec::new(),
        enable_write: false,
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
            "--media-root" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--media-root requires a directory".to_owned());
                };
                parsed.media_roots.push(path.clone());
                index += 2;
            }
            "--enable-write" => {
                parsed.enable_write = true;
                index += 1;
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
    if parsed.enable_write && !cfg!(feature = "workbench-write-dev") {
        return Err(
            "--enable-write is unavailable: this binary was built without the development-only workbench-write-dev capability"
                .to_owned(),
        );
    }
    Ok(parsed)
}

fn serve_sync(args: ServeArgs) -> Result<(), String> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|error| format!("failed to start Tokio runtime: {error}"))?;
    runtime.block_on(serve(args))
}

async fn serve(args: ServeArgs) -> Result<(), String> {
    if args.enable_write {
        recover_workspace(&manifest_root(&args.manifest_path))?;
    }
    let manifest = read_manifest(&args.manifest_path)?;
    let metadata = Arc::new(
        WorkspaceMetadata::load(
            &args.manifest_path,
            manifest,
            &args.media_roots,
            args.enable_write,
        )
        .map_err(WorkbenchError::message)?,
    );
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
        .route("/api/workbench/note-list", get(note_list))
        .route("/api/workbench/note-detail", get(note_detail))
        .route("/api/workbench/card-list", get(card_list))
        .route("/api/workbench/source-string-list", get(source_string_list))
        .route("/api/workbench/metadata-list", get(optional_metadata_list))
        .route(
            "/api/workbench/optional-metadata-list",
            get(optional_metadata_list),
        )
        .route("/api/workbench/note-pivot", get(note_pivot))
        .route(
            "/api/workbench/source-string-pivot",
            get(source_string_pivot),
        )
        .route("/api/workbench/card-pivot", get(card_pivot))
        .route("/api/workbench/comparison-pane", get(comparison_pane))
        .route("/api/workbench/metadata", get(optional_metadata))
        .route("/api/workbench/optional-metadata", get(optional_metadata))
        .route(
            "/api/workbench/new-language-preview",
            get(new_language_preview),
        )
        .route("/api/workbench/new-language", post(create_new_language))
        .route("/api/workbench/apply-preview", post(apply_preview))
        .route("/api/workbench/apply", post(apply_edits))
        .route("/api/media/{*path}", get(media_asset))
        .route("/favicon.ico", get(favicon))
        .with_state(metadata);

    if let Some(assets) = dev_assets {
        router.fallback_service(
            ServeDir::new(&assets).not_found_service(ServeFile::new(assets.join("index.html"))),
        )
    } else {
        router.fallback(get(embedded_asset))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkbenchErrorKind {
    InvalidRequest,
    NotFound,
    Conflict,
    ReadOnly,
    Domain,
    Adapter,
    Internal,
}

impl WorkbenchErrorKind {
    fn status(self) -> StatusCode {
        match self {
            Self::InvalidRequest => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict => StatusCode::CONFLICT,
            Self::ReadOnly => StatusCode::FORBIDDEN,
            Self::Domain => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Adapter | Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn code(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::NotFound => "not_found",
            Self::Conflict => "workspace_conflict",
            Self::ReadOnly => "workbench_read_only",
            Self::Domain => "domain_failed",
            Self::Adapter => "adapter_error",
            Self::Internal => "workbench_internal_error",
        }
    }

    fn category(self) -> &'static str {
        match self {
            Self::InvalidRequest => "request",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::ReadOnly => "authorization",
            Self::Domain => "domain",
            Self::Adapter => "adapter",
            Self::Internal => "internal",
        }
    }
}

#[derive(Debug)]
struct WorkbenchError {
    kind: WorkbenchErrorKind,
    message: String,
    diagnostics: Vec<DomainDiagnostic>,
}

type WorkbenchResult<T> = Result<T, WorkbenchError>;
type WorkbenchCoreResult<T> = Result<T, WorkbenchError>;

impl WorkbenchError {
    fn new(kind: WorkbenchErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            diagnostics: Vec::new(),
        }
    }

    fn request(message: impl Into<String>) -> Self {
        Self::new(WorkbenchErrorKind::InvalidRequest, message)
    }
    fn not_found(message: impl Into<String>) -> Self {
        Self::new(WorkbenchErrorKind::NotFound, message)
    }
    fn conflict(message: impl Into<String>) -> Self {
        Self::new(WorkbenchErrorKind::Conflict, message)
    }
    fn adapter(message: impl Into<String>) -> Self {
        Self::new(WorkbenchErrorKind::Adapter, message)
    }
    fn internal(message: impl Into<String>) -> Self {
        Self::new(WorkbenchErrorKind::Internal, message)
    }
    fn message(self) -> String {
        self.message
    }

    fn domain(message: impl Into<String>, diagnostics: Vec<DomainDiagnostic>) -> Self {
        Self {
            kind: WorkbenchErrorKind::Domain,
            message: message.into(),
            diagnostics,
        }
    }

    fn compose(message: impl Into<String>, report: brain_brew_core::ComposeReport) -> Self {
        Self::domain(
            message,
            report
                .errors
                .iter()
                .map(|error| error.diagnostic())
                .collect(),
        )
    }

    fn field_graph(message: impl Into<String>, report: FieldGraphReport) -> Self {
        Self::domain(
            message,
            report
                .errors
                .iter()
                .map(|error| error.diagnostic())
                .collect(),
        )
    }

    fn render(message: impl Into<String>, report: brain_brew_core::VariableRenderReport) -> Self {
        Self::domain(message, report.diagnostics())
    }
}

impl From<String> for WorkbenchError {
    fn from(message: String) -> Self {
        Self::adapter(message)
    }
}

impl IntoResponse for WorkbenchError {
    fn into_response(self) -> Response {
        let code = if self.kind == WorkbenchErrorKind::Domain {
            self.diagnostics
                .first()
                .map(|item| item.code)
                .unwrap_or(self.kind.code())
        } else {
            self.kind.code()
        };
        let category = if self.kind == WorkbenchErrorKind::Domain {
            self.diagnostics
                .first()
                .map(|item| item.category.as_str())
                .unwrap_or(self.kind.category())
        } else {
            self.kind.category()
        };
        let diagnostics = self
            .diagnostics
            .iter()
            .map(crate::output::diagnostic_json)
            .collect::<Vec<_>>();
        (
            self.kind.status(),
            Json(json!({
                "error": {
                    "schema_version": crate::output::DIAGNOSTIC_SCHEMA_VERSION,
                    "code": code,
                    "category": category,
                    "message": self.message,
                    "diagnostics": diagnostics,
                }
            })),
        )
            .into_response()
    }
}

async fn health(State(metadata): State<Arc<WorkspaceMetadata>>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "manifest": metadata.manifest_path.display().to_string(),
    }))
}

async fn workspace(State(metadata): State<Arc<WorkspaceMetadata>>) -> WorkbenchResult<Json<Value>> {
    task::spawn_blocking(move || metadata.workspace_json().map(Json))
        .await
        .map_err(|error| WorkbenchError::internal(format!("workbench task failed: {error}")))?
}

async fn note_list(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Query(query): Query<NoteListQuery>,
) -> WorkbenchResult<Json<Value>> {
    run_workbench_blocking(move || metadata.note_list_json(&query).map(Json)).await
}

async fn note_detail(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Query(query): Query<NoteDetailQuery>,
) -> WorkbenchResult<Json<Value>> {
    run_workbench_blocking(move || metadata.note_detail_json(&query).map(Json)).await
}

async fn card_list(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Query(query): Query<CardListQuery>,
) -> WorkbenchResult<Json<Value>> {
    run_workbench_blocking(move || metadata.card_list_json(&query).map(Json)).await
}

async fn source_string_list(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Query(query): Query<SourceStringListQuery>,
) -> WorkbenchResult<Json<Value>> {
    run_workbench_blocking(move || metadata.source_string_list_json(&query).map(Json)).await
}

async fn optional_metadata_list(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Query(query): Query<OptionalMetadataListQuery>,
) -> WorkbenchResult<Json<Value>> {
    run_workbench_blocking(move || metadata.optional_metadata_list_json(&query).map(Json)).await
}

async fn note_pivot(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Query(query): Query<NotePivotQuery>,
) -> WorkbenchResult<Json<Value>> {
    run_workbench_blocking(move || metadata.note_pivot_json(&query).map(Json)).await
}

async fn source_string_pivot(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Query(query): Query<SourceStringPivotQuery>,
) -> WorkbenchResult<Json<Value>> {
    run_workbench_blocking(move || metadata.source_string_pivot_json(&query).map(Json)).await
}

async fn card_pivot(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Query(query): Query<CardPivotQuery>,
) -> WorkbenchResult<Json<Value>> {
    run_workbench_blocking(move || metadata.card_pivot_json(&query).map(Json)).await
}

async fn comparison_pane(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Query(query): Query<ComparisonPaneQuery>,
) -> WorkbenchResult<Json<Value>> {
    run_workbench_blocking(move || metadata.comparison_pane_json(&query).map(Json)).await
}

async fn optional_metadata(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Query(query): Query<OptionalMetadataQuery>,
) -> WorkbenchResult<Json<Value>> {
    run_workbench_blocking(move || metadata.optional_metadata_json(&query).map(Json)).await
}

async fn new_language_preview(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Query(query): Query<NewLanguagePreviewQuery>,
) -> WorkbenchResult<Json<Value>> {
    run_workbench_blocking(move || metadata.new_language_preview_json(&query).map(Json)).await
}

async fn create_new_language(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Json(request): Json<NewLanguageRequest>,
) -> WorkbenchResult<Json<Value>> {
    metadata.require_write_capability()?;
    run_workbench_blocking(move || metadata.create_new_language_json(request).map(Json)).await
}

async fn apply_preview(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Json(request): Json<ApplyRequest>,
) -> WorkbenchResult<Json<Value>> {
    run_workbench_blocking(move || {
        metadata
            .apply_request_json(request, ApplyMode::Preview)
            .map(Json)
    })
    .await
}

async fn apply_edits(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    Json(request): Json<ApplyRequest>,
) -> WorkbenchResult<Json<Value>> {
    metadata.require_write_capability()?;
    run_workbench_blocking(move || {
        metadata
            .apply_request_json(request, ApplyMode::Write)
            .map(Json)
    })
    .await
}

async fn media_asset(
    State(metadata): State<Arc<WorkspaceMetadata>>,
    AxumPath(path): AxumPath<String>,
) -> WorkbenchResult<Response> {
    run_workbench_blocking(move || metadata.media_response(&path)).await
}

async fn favicon() -> Response {
    Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

async fn run_workbench_blocking<T, F>(operation: F) -> WorkbenchResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> WorkbenchCoreResult<T> + Send + 'static,
{
    task::spawn_blocking(operation)
        .await
        .map_err(|error| WorkbenchError::internal(format!("workbench task failed: {error}")))?
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

struct WorkspaceMetadata {
    manifest_path: PathBuf,
    manifest_root: PathBuf,
    media_roots: MediaRootSelections,
    write_enabled: bool,
    cache: tokio::sync::RwLock<WorkspaceCache>,
    apply_mutex: Mutex<()>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileSignature {
    len: u64,
    modified: Option<SystemTime>,
}

#[derive(Clone)]
struct CachedValue<T> {
    generation: u64,
    value: T,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SelectionCacheKey {
    language_code: String,
    target_label: String,
    overlay_label: String,
}

struct WorkspaceCache {
    generation: u64,
    manifest: FederatedDeckManifest,
    file_signatures: BTreeMap<PathBuf, Option<FileSignature>>,
    declared_media_paths: Option<CachedValue<BTreeMap<String, MediaDeclarationProvenance>>>,
    selected_contexts: BTreeMap<SelectionCacheKey, CachedValue<SelectedTranslationContext>>,
}

impl WorkspaceCache {
    fn new(manifest: FederatedDeckManifest) -> Self {
        Self {
            generation: 0,
            manifest,
            file_signatures: BTreeMap::new(),
            declared_media_paths: None,
            selected_contexts: BTreeMap::new(),
        }
    }

    fn clear_generation_caches(&mut self) {
        self.declared_media_paths = None;
        self.selected_contexts.clear();
    }
}

impl WorkspaceMetadata {
    fn load(
        manifest_path: &Path,
        manifest: FederatedDeckManifest,
        media_root_args: &[String],
        enable_write: bool,
    ) -> WorkbenchCoreResult<Self> {
        let root = manifest_root(manifest_path);
        let registry = ManifestRegistry::load(manifest_path, &[], &[])?;
        let media_roots = MediaRootSelections::parse(&registry, media_root_args, &root)?;
        Ok(Self {
            manifest_path: manifest_path.to_path_buf(),
            manifest_root: root,
            media_roots,
            write_enabled: cfg!(feature = "workbench-write-dev") && enable_write,
            cache: tokio::sync::RwLock::new(WorkspaceCache::new(manifest)),
            apply_mutex: Mutex::new(()),
        })
    }

    fn require_write_capability(&self) -> WorkbenchResult<()> {
        if self.write_enabled {
            Ok(())
        } else {
            Err(WorkbenchError::new(
                WorkbenchErrorKind::ReadOnly,
                "Workbench is read-only; source mutation is unavailable in this server",
            ))
        }
    }

    fn lock_apply(&self) -> WorkbenchCoreResult<MutexGuard<'_, ()>> {
        self.apply_mutex
            .lock()
            .map_err(|_| WorkbenchError::internal("workbench apply lock poisoned"))
    }

    fn current_manifest(&self) -> WorkbenchCoreResult<FederatedDeckManifest> {
        self.ensure_fresh_cache()?;
        Ok(self.cache.blocking_read().manifest.clone())
    }

    fn current_manifest_snapshot(&self) -> WorkbenchCoreResult<(FederatedDeckManifest, Vec<u8>)> {
        let bytes = fs::read(&self.manifest_path)
            .map_err(|error| format!("{}: {error}", self.manifest_path.display()))?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|error| format!("{}: {error}", self.manifest_path.display()))?;
        let manifest = manifest::from_str(text)
            .map_err(|error| format!("{}: {error}", self.manifest_path.display()))?;
        Ok((manifest, bytes))
    }

    fn ensure_fresh_cache(&self) -> WorkbenchCoreResult<()> {
        let (generation, manifest, previous_signatures) = {
            let cache = self.cache.blocking_read();
            (
                cache.generation,
                cache.manifest.clone(),
                cache.file_signatures.clone(),
            )
        };
        let current_signatures = self.tracked_file_signatures(&manifest)?;
        if previous_signatures.is_empty() {
            let mut cache = self.cache.blocking_write();
            if cache.file_signatures.is_empty() {
                cache.file_signatures = self.tracked_file_signatures(&cache.manifest)?;
            }
            return Ok(());
        }
        if current_signatures == previous_signatures {
            return Ok(());
        }

        let manifest_changed = current_signatures.get(&self.manifest_path)
            != previous_signatures.get(&self.manifest_path);
        let refreshed_manifest = if manifest_changed {
            read_manifest(&self.manifest_path)?
        } else {
            manifest
        };
        let refreshed_signatures = self.tracked_file_signatures(&refreshed_manifest)?;

        let mut cache = self.cache.blocking_write();
        if cache.generation == generation && cache.file_signatures == previous_signatures {
            cache.generation += 1;
            cache.manifest = refreshed_manifest;
            cache.file_signatures = refreshed_signatures;
            cache.clear_generation_caches();
        }
        Ok(())
    }

    fn invalidate_workspace_cache_after_write(&self) -> WorkbenchCoreResult<()> {
        let manifest = read_manifest(&self.manifest_path)?;
        let signatures = self.tracked_file_signatures(&manifest)?;
        let mut cache = self.cache.blocking_write();
        cache.generation += 1;
        cache.manifest = manifest;
        cache.file_signatures = signatures;
        cache.clear_generation_caches();
        Ok(())
    }

    fn tracked_file_signatures(
        &self,
        manifest: &FederatedDeckManifest,
    ) -> WorkbenchCoreResult<BTreeMap<PathBuf, Option<FileSignature>>> {
        let paths =
            authorized_manifest_source_files(&self.manifest_path, &self.manifest_root, manifest)?;
        Ok(paths
            .into_iter()
            .map(|path| {
                let signature = fs::metadata(&path).ok().map(|metadata| FileSignature {
                    len: metadata.len(),
                    modified: metadata.modified().ok(),
                });
                (path, signature)
            })
            .collect())
    }

    fn workspace_json(&self) -> WorkbenchCoreResult<Value> {
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
                "metadata_categories": manifest.translation_profile.metadata_categories.iter().map(metadata_category_json).collect::<Vec<_>>(),
                "metadata_paths": manifest.translation_profile.metadata_paths,
                "metadata_exclude_paths": manifest.translation_profile.metadata_exclude_paths,
                "metadata_category_order": manifest.translation_profile.metadata_category_order,
            },
            "fingerprints": fingerprints,
            "write_capability": {
                "enabled": self.write_enabled,
                "mode": if self.write_enabled { "development_write" } else { "read_only" },
                "development_build": cfg!(feature = "workbench-write-dev"),
                "runtime_opt_in": self.write_enabled,
                "warning": if self.write_enabled {
                    "DEVELOPMENT WRITE MODE: source-document and recoverable transaction guards are enabled, but complete request CAS, preview binding, and security gates remain incomplete. Do not use on irreplaceable work."
                } else {
                    "Workbench is read-only while complete compare-and-swap fingerprints, bound preview confirmation, and applicable security gates are incomplete."
                },
            },
        }))
    }

    fn note_list_json(&self, query: &NoteListQuery) -> WorkbenchCoreResult<Value> {
        let manifest = self.current_manifest()?;
        let pagination = parse_pagination(query.limit.as_deref(), query.offset.as_deref())?;
        let context = self.selected_translation_context(
            &manifest,
            query.language.as_deref(),
            query.target.as_deref(),
            query.overlay.as_deref(),
        )?;
        Ok(note_list_json_from_context(
            &context,
            &manifest,
            query.filter.as_deref(),
            pagination,
        ))
    }

    fn note_detail_json(&self, query: &NoteDetailQuery) -> WorkbenchCoreResult<Value> {
        let manifest = self.current_manifest()?;
        let note = query
            .note
            .as_deref()
            .ok_or_else(|| "invalid note detail request: missing note".to_owned())?;
        let context = self.selected_translation_context(
            &manifest,
            query.language.as_deref(),
            query.target.as_deref(),
            query.overlay.as_deref(),
        )?;
        let detail = note_pivot_json_from_context(&context, &manifest, None, Some(note));
        if detail["notes"].as_array().is_none_or(Vec::is_empty) {
            return Err(WorkbenchError::not_found(format!("unknown note {note:?}")));
        }
        Ok(detail)
    }

    fn card_list_json(&self, query: &CardListQuery) -> WorkbenchCoreResult<Value> {
        let manifest = self.current_manifest()?;
        let pagination = parse_pagination(query.limit.as_deref(), query.offset.as_deref())?;
        let context = self.selected_translation_context(
            &manifest,
            query.language.as_deref(),
            query.target.as_deref(),
            query.overlay.as_deref(),
        )?;
        Ok(card_list_json_from_context(
            &context,
            &manifest,
            query.filter.as_deref(),
            query.content_group.as_deref(),
            pagination,
        ))
    }

    fn source_string_list_json(&self, query: &SourceStringListQuery) -> WorkbenchCoreResult<Value> {
        let manifest = self.current_manifest()?;
        let pagination = parse_pagination(query.limit.as_deref(), query.offset.as_deref())?;
        let context = self.selected_translation_context(
            &manifest,
            query.language.as_deref(),
            query.target.as_deref(),
            query.overlay.as_deref(),
        )?;
        Ok(source_string_list_json_from_context(
            &context,
            &manifest,
            query.content_group.as_deref(),
            query.status.as_deref(),
            pagination,
        ))
    }

    fn optional_metadata_list_json(
        &self,
        query: &OptionalMetadataListQuery,
    ) -> WorkbenchCoreResult<Value> {
        let manifest = self.current_manifest()?;
        let pagination = parse_pagination(query.limit.as_deref(), query.offset.as_deref())?;
        let context = self.selected_translation_context(
            &manifest,
            query.language.as_deref(),
            query.target.as_deref(),
            query.overlay.as_deref(),
        )?;
        Ok(optional_metadata_list_json_from_context(
            &context, &manifest, pagination,
        ))
    }

    fn note_pivot_json(&self, query: &NotePivotQuery) -> WorkbenchCoreResult<Value> {
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

    fn source_string_pivot_json(
        &self,
        query: &SourceStringPivotQuery,
    ) -> WorkbenchCoreResult<Value> {
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

    fn card_pivot_json(&self, query: &CardPivotQuery) -> WorkbenchCoreResult<Value> {
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

    fn optional_metadata_json(&self, query: &OptionalMetadataQuery) -> WorkbenchCoreResult<Value> {
        let manifest = self.current_manifest()?;
        let context = self.selected_translation_context(
            &manifest,
            query.language.as_deref(),
            query.target.as_deref(),
            query.overlay.as_deref(),
        )?;
        Ok(optional_metadata_json_from_context(&context, &manifest))
    }

    fn comparison_pane_json(&self, query: &ComparisonPaneQuery) -> WorkbenchCoreResult<Value> {
        let manifest = self.current_manifest()?;
        let context = self.selected_translation_context(
            &manifest,
            query.language.as_deref(),
            query.target.as_deref(),
            query.overlay.as_deref(),
        )?;
        Ok(json!({
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
            },
            "target_label": context.selection.target_label,
            "content_groups": main_field_rows(&context.source_deck, &context.selection, &context.report.entries)
                .iter()
                .flat_map(|row| content_group_badges_for_note(&context.source_deck, &row.note_id))
                .collect::<BTreeSet<_>>(),
            "note_pivot": note_pivot_json_from_context(&context, &manifest, None, None),
            "source_string_pivot": source_string_pivot_json_from_context(
                &context,
                &manifest,
                query.source.as_deref(),
                query.content_group.as_deref(),
                None,
            ),
            "card_pivot": card_pivot_json_from_context(
                &context,
                &manifest,
                query.card.as_deref(),
                None,
                query.content_group.as_deref(),
            ),
        }))
    }

    fn new_language_preview_json(
        &self,
        query: &NewLanguagePreviewQuery,
    ) -> WorkbenchCoreResult<Value> {
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
        new_language_preview_response(&updated, &request, &overlay_writes)
    }

    fn create_new_language_json(&self, request: NewLanguageRequest) -> WorkbenchCoreResult<Value> {
        let _apply_guard = self.lock_apply()?;
        recover_workspace(&self.manifest_root)?;
        let (manifest, manifest_original) = self.current_manifest_snapshot()?;
        let (updated, overlay_writes) =
            apply_new_language_request(&self.manifest_root, &manifest, &request)?;
        let manifest_yaml = manifest::to_string(&updated).map_err(|error| error.to_string())?;
        manifest::from_str(&manifest_yaml)
            .map_err(|error| format!("invalid generated manifest: {error}"))?;

        let mut outputs = BTreeMap::new();
        for (relative_path, overlay) in overlay_writes {
            let path = PathAuthorizer::new("workspace", &self.manifest_root)?
                .authorize_create(
                    &self.manifest_path,
                    "new-language overlay file",
                    &relative_path,
                )
                .map_err(|error| error.to_string())?
                .into_path_buf();
            let provenance = brain_brew_formats::source_document::SourceProvenance::new(
                path.display().to_string(),
            )
            .with_source_root(self.manifest_root.display().to_string());
            let emission = OverlaySourceDocument::from_overlay(provenance, overlay)
                .and_then(|document| document.emit())
                .map_err(|error| format!("invalid generated overlay {relative_path}: {error}"))?;
            collect_document_emission(emission, &mut outputs)?;
        }
        if outputs
            .insert(
                self.manifest_path.clone(),
                PlannedSourceOutput {
                    original: OriginalSourceSnapshot::Present(manifest_original),
                    replacement: manifest_yaml.into_bytes(),
                },
            )
            .is_some()
        {
            return Err((format!(
                "new-language overlay output conflicts with manifest {}",
                self.manifest_path.display()
            ))
            .into());
        }
        let writes = planned_workbench_outputs(&self.manifest_root, outputs)?;
        commit_workspace_files(&self.manifest_root, writes).map_err(WorkbenchError::conflict)?;
        self.invalidate_workspace_cache_after_write()?;

        Ok(json!({
            "created": true,
            "language": request.code,
            "workspace": self.workspace_json()?,
        }))
    }

    fn apply_request_json(
        &self,
        request: ApplyRequest,
        mode: ApplyMode,
    ) -> WorkbenchCoreResult<Value> {
        let _apply_guard = if mode == ApplyMode::Write {
            Some(self.lock_apply()?)
        } else {
            None
        };
        if mode == ApplyMode::Write {
            recover_workspace(&self.manifest_root)?;
        }
        let manifest = self.current_manifest()?;
        let context = self.selected_translation_context(
            &manifest,
            request.language.as_deref(),
            request.target.as_deref(),
            request.overlay.as_deref(),
        )?;
        if request.edits.is_empty() {
            return Err(WorkbenchError::request("missing staged edits"));
        }

        let source_edits = request
            .edits
            .iter()
            .filter(|edit| edit.kind == WorkbenchEditKind::Source)
            .cloned()
            .collect::<Vec<_>>();
        let mut translation_groups = BTreeMap::<
            (String, String, String),
            (SelectedTranslationContext, Vec<StagedWorkbenchEdit>),
        >::new();
        for edit in request
            .edits
            .iter()
            .filter(|edit| edit.kind == WorkbenchEditKind::Translation)
        {
            let group_context = self.selected_translation_context(
                &manifest,
                edit.language.as_deref().or(request.language.as_deref()),
                edit.target.as_deref().or(request.target.as_deref()),
                edit.overlay.as_deref().or(request.overlay.as_deref()),
            )?;
            let key = (
                group_context.selection.language_code.clone(),
                group_context.selection.target_label.clone(),
                group_context.selection.overlay_label.clone(),
            );
            translation_groups
                .entry(key)
                .or_insert_with(|| (group_context, Vec::new()))
                .1
                .push(edit.clone());
        }

        let primary_planned = planned_overlay_for_path(&context, &context.selection.overlay_file)?;
        ensure_root_source_mutable("Workbench translation edit", &primary_planned.source)?;
        for include in &primary_planned.includes {
            ensure_root_source_mutable("Workbench included overlay edit", include)?;
        }
        let mut primary_overlay_document = read_overlay_document(&context.selection.overlay_file)?;
        let mut primary_overlay = primary_overlay_document.resolved_overlay().clone();
        let mut modified_base = context.base_deck.clone();
        let base_file = context.base_source.path.clone();
        let mut source_plan = apply_staged_source_edits(
            &mut modified_base,
            &mut primary_overlay,
            &mut primary_overlay_document,
            &source_edits,
            &context,
            &base_file,
            &self.manifest_root,
        )?;

        let mut validation_contexts = vec![context.clone()];
        let mut changes = Vec::new();
        for change in std::mem::take(&mut source_plan.changed_entries) {
            let file = if change["mode"] == "source" {
                workspace_path(&self.manifest_root, &base_file)
            } else {
                context.selection.overlay_display_file.clone()
            };
            changes.push(annotated_apply_change(
                change,
                &file,
                &context,
                &modified_base,
            ));
        }

        let mut validation =
            validate_modified_base_and_overlay(&context, &modified_base, &primary_overlay)?;
        let mut overlay_writes =
            BTreeMap::<PathBuf, (String, OverlaySourceDocument, Overlay, bool)>::new();
        if source_plan.overlay_changed {
            overlay_writes.insert(
                context.selection.overlay_file.clone(),
                (
                    context.selection.overlay_display_file.clone(),
                    primary_overlay_document.clone(),
                    primary_overlay.clone(),
                    true,
                ),
            );
        }

        for (_, (group_context, translation_edits)) in translation_groups {
            validation_contexts.push(group_context.clone());
            let planned =
                planned_overlay_for_path(&group_context, &group_context.selection.overlay_file)?;
            ensure_root_source_mutable("Workbench translation edit", &planned.source)?;
            for include in &planned.includes {
                ensure_root_source_mutable("Workbench included overlay edit", include)?;
            }
            let (mut document, mut overlay) =
                if group_context.selection.overlay_file == context.selection.overlay_file {
                    (primary_overlay_document.clone(), primary_overlay.clone())
                } else {
                    let document = read_overlay_document(&group_context.selection.overlay_file)?;
                    let overlay = document.resolved_overlay().clone();
                    (document, overlay)
                };
            let context_after_source = context_with_modified_base_and_overlay(
                &group_context,
                modified_base.clone(),
                &overlay,
            )?;
            let group_changes = apply_staged_edits_to_overlay(
                &mut overlay,
                &mut document,
                &translation_edits,
                &context_after_source,
            )?;
            for change in group_changes {
                changes.push(annotated_apply_change(
                    change,
                    &group_context.selection.overlay_display_file,
                    &group_context,
                    &modified_base,
                ));
            }
            let group_validation =
                validate_modified_base_and_overlay(&group_context, &modified_base, &overlay)?;
            validation.extend(group_validation.diagnostics);
            overlay_writes.insert(
                group_context.selection.overlay_file.clone(),
                (
                    group_context.selection.overlay_display_file.clone(),
                    document,
                    overlay,
                    !translation_edits.is_empty(),
                ),
            );
        }

        if changes.is_empty() {
            return Err(WorkbenchError::request("missing staged edits"));
        }
        let mut outputs = source_plan.outputs.clone();
        for (display_file, document, _overlay, changed) in overlay_writes.values() {
            if !changed {
                continue;
            }
            collect_document_emission(
                document.emit().map_err(|error| {
                    format!("invalid generated translation overlay {display_file}: {error}")
                })?,
                &mut outputs,
            )?;
        }
        validation.extend(validate_complete_workbench_result(
            &modified_base,
            &overlay_writes,
            &validation_contexts,
        )?);
        if mode == ApplyMode::Write && !validation.diagnostics.is_empty() {
            return Err(WorkbenchError::domain(
                "validation failed; preview before applying",
                validation.diagnostics,
            ));
        }
        if mode == ApplyMode::Write {
            let writes = planned_workbench_outputs(&self.manifest_root, outputs)?;
            commit_workspace_files(&self.manifest_root, writes)
                .map_err(WorkbenchError::conflict)?;
            self.invalidate_workspace_cache_after_write()?;
        }

        let mut affected_files = source_plan.affected_files_json(&self.manifest_root);
        for (path, (display_file, _document, _overlay, changed)) in &overlay_writes {
            if *changed {
                push_unique_affected_file(
                    &mut affected_files,
                    &self.manifest_root,
                    path,
                    Some(display_file),
                );
            }
        }
        let file_groups = apply_file_groups_json(&changes);
        let validation_ok = validation.diagnostics.is_empty();
        let validation_diagnostics = validation
            .diagnostics
            .iter()
            .map(crate::output::diagnostic_json)
            .collect::<Vec<_>>();

        Ok(json!({
            "mode": mode.as_str(),
            "applied": mode == ApplyMode::Write,
            "language": context.selection.language_code,
            "target_label": context.selection.target_label,
            "target_id": context.selection.target_id,
            "overlay_label": context.selection.overlay_label,
            "overlay_id": context.selection.overlay_id,
            "affected_files": affected_files,
            "file_groups": file_groups,
            "changed_entries": changes,
            "validation": {
                "schema_version": crate::output::DIAGNOSTIC_SCHEMA_VERSION,
                "ok": validation_ok,
                "diagnostics": validation_diagnostics,
            },
        }))
    }

    fn media_response(&self, requested_path: &str) -> WorkbenchCoreResult<Response> {
        let requested_path = safe_media_relative_path(requested_path)?;
        let declaration = self.media_declaration(&requested_path)?.ok_or_else(|| {
            WorkbenchError::not_found(format!("unknown media asset {requested_path:?}"))
        })?;
        let root = if let Some(root) = self.media_roots.explicit_for_declaration(&declaration) {
            root.to_path_buf()
        } else if self.media_roots.supplied() {
            self.media_roots
                .require_for_declaration("Workbench media catalog", &declaration)?
                .to_path_buf()
        } else {
            // Compatibility fallback is owner-derived, never target-root-derived:
            // package_root/media wins when present, otherwise package_root.
            let conventional = declaration.package_root.join("media");
            if conventional.is_dir() {
                conventional
            } else {
                declaration.package_root.clone()
            }
        };
        let authorizer = PathAuthorizer::new(
            format!(
                "Workbench media for package {}",
                declaration.package_label()
            ),
            &root,
        )?;
        match authorizer.authorize_read(
            &declaration.source,
            format!("media.{}.path", declaration.id),
            &requested_path,
        ) {
            Ok(path) => {
                let path = path.into_path_buf();
                let bytes =
                    fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, media_content_type(&requested_path))
                    .body(Body::from(bytes))
                    .map_err(|error| {
                        WorkbenchError::internal(format!("failed to build media response: {error}"))
                    })
            }
            Err(_error) if !root.join(&requested_path).exists() => {
                let placeholder = missing_media_placeholder_svg(&requested_path);
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "image/svg+xml")
                    .body(Body::from(placeholder))
                    .map_err(|error| {
                        WorkbenchError::internal(format!(
                            "failed to build missing media placeholder: {error}"
                        ))
                    })
            }
            Err(error) => Err(WorkbenchError::adapter(format!(
                "Workbench media ownership error for package {}, declaration {}, path {:?}, root {}: {error}",
                declaration.package_label(),
                declaration.id,
                declaration.path,
                root.display()
            ))),
        }
    }

    fn media_declaration(
        &self,
        requested_path: &str,
    ) -> WorkbenchCoreResult<Option<MediaDeclarationProvenance>> {
        self.ensure_fresh_cache()?;
        let (generation, manifest, cached) = {
            let cache = self.cache.blocking_read();
            if let Some(declaration) = cache
                .selected_contexts
                .values()
                .filter(|context| context.generation == cache.generation)
                .find_map(|context| {
                    context
                        .value
                        .media_declarations
                        .values()
                        .find(|declaration| declaration.path == requested_path)
                        .cloned()
                })
            {
                return Ok(Some(declaration));
            }
            (
                cache.generation,
                cache.manifest.clone(),
                cache.declared_media_paths.clone(),
            )
        };
        if let Some(cached) = cached
            && cached.generation == generation
        {
            return Ok(cached.value.get(requested_path).cloned());
        }

        let mut cache = self.cache.blocking_write();
        if let Some(cached) = &cache.declared_media_paths
            && cached.generation == cache.generation
        {
            return Ok(cached.value.get(requested_path).cloned());
        }
        if cache.generation != generation {
            drop(cache);
            return self.media_declaration(requested_path);
        }
        let declarations = self.collect_declared_media_paths(&manifest)?;
        let declaration = declarations.get(requested_path).cloned();
        cache.declared_media_paths = Some(CachedValue {
            generation,
            value: declarations,
        });
        Ok(declaration)
    }

    fn collect_declared_media_paths(
        &self,
        manifest: &FederatedDeckManifest,
    ) -> WorkbenchCoreResult<BTreeMap<String, MediaDeclarationProvenance>> {
        let mut paths = BTreeMap::new();
        let registry = ManifestRegistry::load(&self.manifest_path, &[], &[])?;
        for target_id in manifest.targets.keys() {
            let reference = manifest
                .package
                .as_ref()
                .map(|package| format!("{}:{target_id}", package.id))
                .unwrap_or_else(|| target_id.clone());
            let plan = registry.plan(&reference)?;
            for declaration in plan.media_declarations.values() {
                if let Some(previous) = paths.insert(declaration.path.clone(), declaration.clone())
                    && (previous.id != declaration.id
                        || previous.package_root != declaration.package_root
                        || previous.source != declaration.source)
                {
                    return Err((format!(
                        "ambiguous Workbench media path {:?}: declaration {} owned by package {} at {} conflicts with declaration {} owned by package {} at {}",
                        declaration.path,
                        previous.id,
                        previous.package_label(),
                        previous.source.display(),
                        declaration.id,
                        declaration.package_label(),
                        declaration.source.display()
                    )).into());
                }
            }
        }
        Ok(paths)
    }

    fn selected_translation_context(
        &self,
        manifest: &FederatedDeckManifest,
        language: Option<&str>,
        target_label: Option<&str>,
        overlay_label: Option<&str>,
    ) -> WorkbenchCoreResult<SelectedTranslationContext> {
        let selection =
            self.select_translation_target(manifest, language, target_label, overlay_label)?;
        let key = SelectionCacheKey {
            language_code: selection.language_code.clone(),
            target_label: selection.target_label.clone(),
            overlay_label: selection.overlay_label.clone(),
        };
        let (generation, cached) = {
            let cache = self.cache.blocking_read();
            (cache.generation, cache.selected_contexts.get(&key).cloned())
        };
        if let Some(cached) = cached
            && cached.generation == generation
        {
            return Ok(cached.value);
        }

        let context = self.build_selected_translation_context(selection)?;
        let mut cache = self.cache.blocking_write();
        if let Some(cached) = cache.selected_contexts.get(&key)
            && cached.generation == cache.generation
        {
            return Ok(cached.value.clone());
        }
        if cache.generation == generation {
            cache.selected_contexts.insert(
                key,
                CachedValue {
                    generation,
                    value: context.clone(),
                },
            );
        }
        Ok(context)
    }

    fn build_selected_translation_context(
        &self,
        selection: WorkbenchSelection,
    ) -> WorkbenchCoreResult<SelectedTranslationContext> {
        let plan = plan_manifest_target(
            &self.manifest_path,
            &selection.target_id,
            &[],
            &[],
            &crate::package_resolver::DiscoveryPolicy::default(),
        )?;
        let mut current = plan.base.clone();
        let mut diagnostic_overlays = Vec::new();
        let mut selected_source_deck = None;
        let mut selected_report = None;
        let mut selected_display_file = selection.overlay_display_file.clone();
        let mut selected_file = selection.overlay_file.clone();

        for (planned, overlay) in &plan.overlays {
            if planned.id == selection.overlay_id {
                selected_display_file = planned.display_file.clone();
                selected_file = planned.file.clone();
                selected_source_deck = Some(current.clone());
                selected_report =
                    Some(current.translation_coverage(overlay).map_err(|report| {
                        WorkbenchError::field_graph(
                            "failed to resolve translation source fields",
                            report,
                        )
                    })?);
            }
            let effective_overlay = if overlay.translations.is_some() {
                sanitize_lenient_translation_overlay(&current, overlay).map_err(|report| {
                    WorkbenchError::field_graph(
                        format!("failed to resolve translation overlay {}", planned.id),
                        report,
                    )
                })?
            } else {
                overlay.clone()
            };
            current = current
                .compose(std::slice::from_ref(&effective_overlay))
                .map_err(|report| {
                    WorkbenchError::compose(
                        format!("failed to compose overlay {}", planned.id),
                        report,
                    )
                })?;
            diagnostic_overlays.push(effective_overlay);
        }
        plan.base.compose(&diagnostic_overlays).map_err(|report| {
            WorkbenchError::compose(
                format!("failed to compose target {}", selection.target_id),
                report,
            )
        })?;

        let Some(source_deck) = selected_source_deck else {
            return Err((format!(
                "target {} does not include translation overlay {}",
                selection.target_id, selection.overlay_id
            ))
            .into());
        };
        let Some(report) = selected_report else {
            return Err((format!(
                "overlay {} is not a translation overlay",
                selection.overlay_id
            ))
            .into());
        };
        Ok(SelectedTranslationContext {
            selection: WorkbenchSelection {
                overlay_file: selected_file,
                overlay_display_file: selected_display_file,
                ..selection
            },
            base_deck: plan.base,
            base_source: plan.base_source,
            base_includes: plan.base_includes,
            plan_overlays: plan.overlays,
            media_declarations: plan.media_declarations,
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
    ) -> WorkbenchCoreResult<WorkbenchSelection> {
        let (language_code, language_entry) = if let Some(language) = language {
            let entry = manifest.languages.get(language).ok_or_else(|| {
                WorkbenchError::not_found(format!("unknown language {language:?}"))
            })?;
            (language.to_owned(), entry)
        } else {
            manifest
                .languages
                .iter()
                .find(|(_, entry)| !entry.source && !entry.translation_overlays.is_empty())
                .map(|(code, entry)| (code.clone(), entry))
                .ok_or_else(|| {
                    WorkbenchError::not_found(
                        "no target language with translation overlays is configured",
                    )
                })?
        };
        if language_entry.source {
            return Err(WorkbenchError::request(format!(
                "invalid source language {language_code:?}; choose a target language"
            )));
        }
        if language_entry.translation_overlays.is_empty() {
            return Err(WorkbenchError::request(format!(
                "language {language_code:?} has no translation overlays"
            )));
        }

        let target_label = target_label
            .map(str::to_owned)
            .or_else(|| {
                (!language_entry.primary_target.is_empty())
                    .then(|| language_entry.primary_target.clone())
            })
            .or_else(|| language_entry.targets.keys().next().cloned())
            .ok_or_else(|| {
                WorkbenchError::not_found(format!("language {language_code:?} has no targets"))
            })?;
        let target_id = language_entry
            .targets
            .get(&target_label)
            .ok_or_else(|| {
                WorkbenchError::not_found(format!(
                    "language {language_code:?} has no target label {target_label:?}"
                ))
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
            .ok_or_else(|| {
                WorkbenchError::not_found(format!(
                    "language {language_code:?} has no translation overlays"
                ))
            })?;
        let overlay_id = language_entry
            .translation_overlays
            .get(&overlay_label)
            .ok_or_else(|| {
                WorkbenchError::not_found(format!(
                    "language {language_code:?} has no overlay label {overlay_label:?}"
                ))
            })?
            .clone();
        let target_reference = if target_id.contains(':') {
            target_id.clone()
        } else {
            manifest
                .package
                .as_ref()
                .map(|package| format!("{}:{target_id}", package.id))
                .unwrap_or_else(|| target_id.clone())
        };
        let plan = plan_manifest_target(
            &self.manifest_path,
            &target_reference,
            &[],
            &[],
            &crate::package_resolver::DiscoveryPolicy::default(),
        )?;
        let planned_overlay = plan
            .overlays
            .iter()
            .find(|(planned, _)| planned.id == overlay_id || planned.qualified_id == overlay_id)
            .map(|(planned, _)| planned)
            .ok_or_else(|| {
                format!(
                    "target {:?} does not include translation overlay {:?}",
                    plan.qualified_name, overlay_id
                )
            })?;
        let overlay_file = planned_overlay.file.clone();
        let overlay_display_file = planned_overlay.display_file.clone();

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
            target_id: plan.qualified_name,
            overlay_label,
            overlay_id,
            overlay_file,
            overlay_display_file,
            overlay_badges,
            structural_fields: manifest
                .translation_profile
                .structural_fields
                .iter()
                .cloned()
                .collect(),
            metadata_categories: manifest.translation_profile.metadata_categories.clone(),
            metadata_exclude_paths: manifest.translation_profile.metadata_exclude_paths.clone(),
        })
    }
}

fn annotated_apply_change(
    mut change: Value,
    file: &str,
    context: &SelectedTranslationContext,
    deck: &CanonicalDeck,
) -> Value {
    let path = change["path"].as_str().unwrap_or("").to_owned();
    change["file"] = json!(file);
    change["file_kind"] = json!(if change["mode"] == "source" {
        "canonical_deck"
    } else {
        "translation_overlay"
    });
    change["language"] = json!(&context.selection.language_code);
    change["target_label"] = json!(&context.selection.target_label);
    change["overlay_label"] = json!(&context.selection.overlay_label);
    change["content_groups"] = json!(content_groups_for_apply_path(deck, &path));
    change
}

fn content_groups_for_apply_path(deck: &CanonicalDeck, path: &str) -> BTreeSet<String> {
    let mut groups = path
        .strip_prefix("notes.")
        .and_then(|suffix| suffix.split_once(".fields."))
        .and_then(|(note_id, _)| StableId::new(note_id.to_owned()).ok())
        .map(|note_id| content_group_badges_for_note(deck, &note_id))
        .unwrap_or_default();
    if groups.is_empty() {
        groups.insert("workspace".to_owned());
    }
    groups
}

fn apply_file_groups_json(changes: &[Value]) -> Value {
    let mut grouped = BTreeMap::<String, (String, BTreeMap<String, usize>)>::new();
    for change in changes {
        let file = change["file"].as_str().unwrap_or("unknown").to_owned();
        let file_kind = change["file_kind"].as_str().unwrap_or("unknown").to_owned();
        let content_groups = change["content_groups"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|group| group.as_str())
            .collect::<Vec<_>>();
        let (_, groups) = grouped
            .entry(file)
            .or_insert_with(|| (file_kind, BTreeMap::new()));
        if content_groups.is_empty() {
            *groups.entry("workspace".to_owned()).or_insert(0) += 1;
        } else {
            for group in content_groups {
                *groups.entry(group.to_owned()).or_insert(0) += 1;
            }
        }
    }
    json!(
        grouped
            .into_iter()
            .map(|(file, (kind, groups))| json!({
                "file": file,
                "kind": kind,
                "content_groups": groups
                    .into_iter()
                    .map(|(name, change_count)| json!({
                        "name": name,
                        "change_count": change_count,
                    }))
                    .collect::<Vec<_>>(),
            }))
            .collect::<Vec<_>>()
    )
}

#[derive(Clone, Debug, Default, Deserialize)]
struct NoteListQuery {
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    filter: Option<String>,
    limit: Option<String>,
    offset: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct NoteDetailQuery {
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    note: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct CardListQuery {
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    filter: Option<String>,
    content_group: Option<String>,
    limit: Option<String>,
    offset: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct SourceStringListQuery {
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    content_group: Option<String>,
    status: Option<String>,
    limit: Option<String>,
    offset: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct OptionalMetadataListQuery {
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    limit: Option<String>,
    offset: Option<String>,
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
struct OptionalMetadataQuery {
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct ComparisonPaneQuery {
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    source: Option<String>,
    card: Option<String>,
    content_group: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct NewLanguagePreviewQuery {
    code: Option<String>,
    display_name: Option<String>,
    template: Option<String>,
}

const DEFAULT_LIST_LIMIT: usize = 50;
const MAX_LIST_LIMIT: usize = 200;

#[derive(Clone, Copy, Debug)]
struct Pagination {
    limit: usize,
    offset: usize,
}

fn parse_pagination(limit: Option<&str>, offset: Option<&str>) -> WorkbenchCoreResult<Pagination> {
    let limit = match limit {
        Some(raw) => raw.parse::<usize>().map_err(|_| {
            WorkbenchError::request(format!(
                "invalid pagination limit {raw:?}: expected an integer from 1 to {MAX_LIST_LIMIT}"
            ))
        })?,
        None => DEFAULT_LIST_LIMIT,
    };
    if limit == 0 || limit > MAX_LIST_LIMIT {
        return Err(WorkbenchError::request(format!(
            "invalid pagination limit {limit}: expected a value from 1 to {MAX_LIST_LIMIT}"
        )));
    }
    let offset = match offset {
        Some(raw) => raw.parse::<usize>().map_err(|_| {
            WorkbenchError::request(format!(
                "invalid pagination offset {raw:?}: expected a non-negative integer"
            ))
        })?,
        None => 0,
    };
    Ok(Pagination { limit, offset })
}

fn paginate<T: Clone>(items: &[T], pagination: Pagination) -> (Vec<T>, bool) {
    let total = items.len();
    let page = items
        .iter()
        .skip(pagination.offset)
        .take(pagination.limit)
        .cloned()
        .collect::<Vec<_>>();
    let has_more = pagination.offset.saturating_add(pagination.limit) < total;
    (page, has_more)
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
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
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
    StaleTranslation,
    MigrateKey,
}

fn default_source_impact_action() -> SourceImpactAction {
    SourceImpactAction::StaleTranslation
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
    metadata_categories: Vec<MetadataCategory>,
    metadata_exclude_paths: Vec<String>,
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
    base_source: PlannedSourceProvenance,
    base_includes: Vec<PlannedSourceProvenance>,
    plan_overlays: Vec<(PlannedOverlay, Overlay)>,
    media_declarations: BTreeMap<String, MediaDeclarationProvenance>,
    source_deck: CanonicalDeck,
    target_deck: CanonicalDeck,
    report: brain_brew_core::TranslationCoverageReport,
}

#[derive(Clone, Debug)]
struct ApplyValidation {
    diagnostics: Vec<DomainDiagnostic>,
}

impl ApplyValidation {
    fn extend(&mut self, diagnostics: impl IntoIterator<Item = DomainDiagnostic>) {
        for diagnostic in diagnostics {
            if !self.diagnostics.contains(&diagnostic) {
                self.diagnostics.push(diagnostic);
            }
        }
    }
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
) -> WorkbenchCoreResult<NewLanguageRequest> {
    let template = manifest.languages.get(template_language).ok_or_else(|| {
        WorkbenchError::request(format!("unknown template language {template_language:?}"))
    })?;
    if template.source || template.translation_overlays.is_empty() {
        return Err(WorkbenchError::request(format!(
            "invalid template language {template_language:?}; choose a target language"
        )));
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
) -> WorkbenchCoreResult<(FederatedDeckManifest, Vec<(String, Overlay)>)> {
    validate_new_language_code(&request.code)?;
    if request.display_name.trim().is_empty() {
        return Err(WorkbenchError::request(
            "invalid new language display name: expected a non-empty value",
        ));
    }
    if manifest.languages.contains_key(&request.code) {
        return Err(WorkbenchError::request(format!(
            "invalid new language code {:?}: language already exists",
            request.code
        )));
    }
    let template = manifest
        .languages
        .get(&request.template_language)
        .ok_or_else(|| {
            WorkbenchError::request(format!(
                "unknown template language {:?}",
                request.template_language
            ))
        })?;
    if template.source || template.translation_overlays.is_empty() {
        return Err(WorkbenchError::request(format!(
            "invalid template language {:?}; choose a target language",
            request.template_language
        )));
    }

    let template_translation_ids = template
        .translation_overlays
        .values()
        .cloned()
        .collect::<BTreeSet<_>>();
    let selected_groups = selected_new_language_groups(template, request)?;
    if selected_groups.is_empty() {
        return Err(WorkbenchError::request(
            "invalid new language scaffold: select at least one translation overlay group",
        ));
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
            return Err(WorkbenchError::request(format!(
                "invalid new language overlay id {:?}: overlay already exists",
                group.overlay_id
            )));
        }
        let absolute_path = PathAuthorizer::new("workspace", manifest_root)?
            .authorize_create(
                &manifest_root.join("brainbrew.yaml"),
                format!("overlays.{}.file", group.overlay_id),
                &group.file,
            )
            .map_err(|error| error.to_string())?
            .into_path_buf();
        if absolute_path.exists() {
            return Err(WorkbenchError::request(format!(
                "invalid new language overlay file already exists: {}",
                absolute_path.display()
            )));
        }
        let template_overlay = manifest
            .overlays
            .get(&group.template_overlay_id)
            .ok_or_else(|| {
                WorkbenchError::request(format!(
                    "unknown template overlay {:?}",
                    group.template_overlay_id
                ))
            })?;
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
            return Err(WorkbenchError::request(format!(
                "invalid new language target id {:?}: target already exists",
                target.target_id
            )));
        }
        let template_target_id = template.targets.get(&target.label).ok_or_else(|| {
            WorkbenchError::request(format!(
                "invalid new language target label {:?}: not found on template language",
                target.label
            ))
        })?;
        let template_target = manifest.targets.get(template_target_id).ok_or_else(|| {
            WorkbenchError::request(format!("unknown template target {template_target_id:?}"))
        })?;
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
        return Err(WorkbenchError::request(
            "invalid new language scaffold: expected at least one target",
        ));
    }
    if !language_targets.contains_key(&request.primary_target) {
        return Err(WorkbenchError::request(format!(
            "invalid new language primary target {:?}: not found in target labels",
            request.primary_target
        )));
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

    let manifest_yaml = manifest::to_string(&updated).map_err(|error| error.to_string())?;
    manifest::from_str(&manifest_yaml)
        .map_err(|error| format!("invalid generated manifest: {error}"))?;

    Ok((updated, overlay_writes))
}

fn selected_new_language_groups<'a>(
    template: &LanguageManifestEntry,
    request: &'a NewLanguageRequest,
) -> WorkbenchCoreResult<Vec<&'a NewLanguageGroupRequest>> {
    let mut seen_labels = BTreeSet::new();
    let mut selected = Vec::new();
    for group in &request.groups {
        if !seen_labels.insert(group.label.clone()) {
            return Err(WorkbenchError::request(format!(
                "invalid new language overlay group {:?}: duplicate label",
                group.label
            )));
        }
        let Some(template_overlay_id) = template.translation_overlays.get(&group.label) else {
            return Err(WorkbenchError::request(format!(
                "invalid new language overlay group {:?}: not found on template language",
                group.label
            )));
        };
        if template_overlay_id != &group.template_overlay_id {
            return Err(WorkbenchError::request(format!(
                "invalid new language overlay group {:?}: template overlay changed",
                group.label
            )));
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
) -> WorkbenchCoreResult<Value> {
    let overlay_files = overlay_writes
        .iter()
        .map(|(path, overlay)| {
            Ok(json!({
                "path": path,
                "contents": canonical_yaml::overlay_to_string(overlay)
                    .map_err(|error| error.to_string())?,
            }))
        })
        .collect::<WorkbenchCoreResult<Vec<_>>>()?;
    let manifest_yaml = manifest::to_string(manifest).map_err(|error| error.to_string())?;
    let mut affected_files = vec![json!({ "path": "brainbrew.yaml" })];
    affected_files.extend(
        overlay_writes
            .iter()
            .map(|(path, _)| json!({ "path": path })),
    );
    Ok(json!({
        "validation": {
            "schema_version": crate::output::DIAGNOSTIC_SCHEMA_VERSION,
            "ok": true,
            "diagnostics": Vec::<Value>::new(),
        },
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
        "manifest_yaml": manifest_yaml,
    }))
}

fn validate_new_language_code(code: &str) -> WorkbenchCoreResult<()> {
    if code.is_empty()
        || !code
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return Err(WorkbenchError::request(format!(
            "invalid new language code {code:?}: expected letters, numbers, '-' or '_'"
        )));
    }
    Ok(())
}

fn validate_stable_id(kind: &str, value: &str) -> WorkbenchCoreResult<()> {
    StableId::new(value.to_owned())
        .map(|_| ())
        .map_err(|error| {
            WorkbenchError::request(format!("invalid new language {kind} {value:?}: {error}"))
        })
}

fn note_list_json_from_context(
    context: &SelectedTranslationContext,
    manifest: &FederatedDeckManifest,
    filter: Option<&str>,
    pagination: Pagination,
) -> Value {
    let entries_by_path = context
        .report
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let progress = main_progress(&context.source_deck, context, &entries_by_path);
    let metadata_progress = optional_metadata_progress(&optional_metadata_rows(context, manifest));
    let rows = note_navigation_rows(context, filter);
    let total = rows.len();
    let (page, has_more) = paginate(&rows, pagination);
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
        },
        "progress": progress,
        "metadata_progress": metadata_progress,
        "tombstones": workbench_tombstones_json(context),
        "total": total,
        "limit": pagination.limit,
        "offset": pagination.offset,
        "has_more": has_more,
        "rows": page,
    })
}

fn card_list_json_from_context(
    context: &SelectedTranslationContext,
    manifest: &FederatedDeckManifest,
    filter: Option<&str>,
    content_group_filter: Option<&str>,
    pagination: Pagination,
) -> Value {
    let content_group_filter = content_group_filter.filter(|filter| *filter != "all");
    let rows = main_field_rows(
        &context.source_deck,
        &context.selection,
        &context.report.entries,
    );
    let all_cards = produced_card_rows(context, &rows);
    let progress_total = all_cards.len();
    let progress_missing = all_cards
        .iter()
        .filter(|card| card.status == "missing")
        .count();
    let progress_stale = all_cards
        .iter()
        .filter(|card| card.status == "stale")
        .count();
    let progress_complete = progress_total.saturating_sub(progress_missing + progress_stale);
    let content_groups = all_cards
        .iter()
        .flat_map(|card| card.content_group_badges.iter().cloned())
        .collect::<BTreeSet<_>>();
    let cards = all_cards
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
    let total = cards.len();
    let summaries = cards.iter().map(card_summary_json).collect::<Vec<_>>();
    let (page, has_more) = paginate(&summaries, pagination);
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
            "total": progress_total,
            "complete": progress_complete,
            "missing": progress_missing,
            "stale": progress_stale,
            "needs_work": progress_missing + progress_stale,
            "percent": progress_percent(progress_complete, progress_total),
        },
        "total": total,
        "limit": pagination.limit,
        "offset": pagination.offset,
        "has_more": has_more,
        "rows": page,
    })
}

fn source_string_list_json_from_context(
    context: &SelectedTranslationContext,
    manifest: &FederatedDeckManifest,
    content_group_filter: Option<&str>,
    status_filter: Option<&str>,
    pagination: Pagination,
) -> Value {
    let content_group_filter = content_group_filter.filter(|filter| *filter != "all");
    let status_filter = status_filter.filter(|filter| *filter != "all");
    let all_rows = source_string_rows(context)
        .into_iter()
        .filter(|row| !row.structural && !row.source.is_empty())
        .collect::<Vec<_>>();
    let all_groups = source_string_groups(&context.source_deck, &all_rows);
    let progress_total = all_groups.len();
    let progress_missing = all_groups
        .iter()
        .filter(|group| group.status == "missing")
        .count();
    let progress_stale = all_groups
        .iter()
        .filter(|group| group.status == "stale")
        .count();
    let progress_complete = all_groups
        .iter()
        .filter(|group| group.status == "complete")
        .count();
    let content_groups = all_rows
        .iter()
        .flat_map(|row| content_group_badges_for_note(&context.source_deck, &row.note_id))
        .collect::<BTreeSet<_>>();
    let rows = all_rows
        .into_iter()
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
    let grouped = source_string_groups(&context.source_deck, &rows)
        .into_iter()
        .filter(|group| status_filter.is_none_or(|filter| filter == group.status))
        .collect::<Vec<_>>();
    let total = grouped.len();
    let summaries = grouped
        .iter()
        .map(|group| source_string_summary_json(group, &source_counts))
        .collect::<Vec<_>>();
    let (page, has_more) = paginate(&summaries, pagination);
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
        "progress": {
            "total": progress_total,
            "complete": progress_complete,
            "missing": progress_missing,
            "stale": progress_stale,
            "needs_work": progress_missing + progress_stale,
            "percent": progress_percent(progress_complete, progress_total),
        },
        "total": total,
        "limit": pagination.limit,
        "offset": pagination.offset,
        "has_more": has_more,
        "rows": page,
    })
}

fn optional_metadata_list_json_from_context(
    context: &SelectedTranslationContext,
    manifest: &FederatedDeckManifest,
    pagination: Pagination,
) -> Value {
    let entries_by_path = context
        .report
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let items = optional_metadata_rows(context, manifest);
    let total = items.len();
    let summaries = items
        .iter()
        .map(optional_metadata_summary_json)
        .collect::<Vec<_>>();
    let (page, has_more) = paginate(&summaries, pagination);
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
        "main_progress": main_progress(&context.source_deck, context, &entries_by_path),
        "metadata_progress": optional_metadata_progress(&items),
        "total": total,
        "limit": pagination.limit,
        "offset": pagination.offset,
        "has_more": has_more,
        "rows": page,
        "profile_metadata_categories": manifest.translation_profile.metadata_categories.iter().map(metadata_category_json).collect::<Vec<_>>(),
        "profile_metadata_paths": manifest.translation_profile.metadata_paths,
        "profile_metadata_exclude_paths": manifest.translation_profile.metadata_exclude_paths,
        "profile_metadata_category_order": manifest.translation_profile.metadata_category_order,
    })
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
    let metadata_progress = optional_metadata_progress(&optional_metadata_rows(context, manifest));
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
        "metadata_progress": metadata_progress,
        "tombstones": workbench_tombstones_json(context),
        "notes": notes,
        "stale_entries": stale_entries_json(&context.report.entries),
    })
}

fn workbench_tombstones_json(context: &SelectedTranslationContext) -> Value {
    let records = |deck: &CanonicalDeck| {
        deck.tombstones
            .iter()
            .map(|record| {
                json!({
                    "kind": record.address.kind(),
                    "path": record.address.to_string(),
                    "removed_by": record.provenance.as_ref().map(|value| value.overlay_id.to_string()),
                    "operation": record.provenance.as_ref().map(|value| value.operation.as_str()),
                })
            })
            .collect::<Vec<_>>()
    };
    json!({
        "source": records(&context.source_deck),
        "target": records(&context.target_deck),
    })
}

fn optional_metadata_json_from_context(
    context: &SelectedTranslationContext,
    manifest: &FederatedDeckManifest,
) -> Value {
    let entries_by_path = context
        .report
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let items = optional_metadata_rows(context, manifest);
    let metadata_progress = optional_metadata_progress(&items);
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
        "main_progress": main_progress(&context.source_deck, context, &entries_by_path),
        "metadata_progress": metadata_progress,
        "items": items.iter().map(optional_metadata_item_json).collect::<Vec<_>>(),
        "profile_metadata_categories": manifest.translation_profile.metadata_categories.iter().map(metadata_category_json).collect::<Vec<_>>(),
        "profile_metadata_paths": manifest.translation_profile.metadata_paths,
        "profile_metadata_exclude_paths": manifest.translation_profile.metadata_exclude_paths,
        "profile_metadata_category_order": manifest.translation_profile.metadata_category_order,
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
                title: note_title(Some(note), Some(note_type)),
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
    let target_fields = context
        .target_deck
        .resolved_field_graph()
        .expect("selected Workbench target deck was graph-validated when its context was built");
    let source_note = context.source_deck.notes.get(&card.note_id);
    let target_note = context.target_deck.notes.get(&card.note_id).or(source_note);
    let source_note_type = context.source_deck.note_types.get(&card.note_type_id);
    let target_note_type = target_note
        .and_then(|note| context.target_deck.note_types.get(&note.note_type_id))
        .or(source_note_type);
    let fields = card
        .field_rows
        .iter()
        .map(|row| {
            let target = target_note
                .map(|note| {
                    resolved_field_text_or_diagnostic(&target_fields, &note.id, &row.field_id)
                })
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
    let rendered_deck = match deck.render_variables() {
        Ok(rendered) => rendered,
        Err(error) => return json!({ "error": error.to_string(), "cards": [] }),
    };
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
        .expect("selected translation context was graph-validated when it was built")
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
    let target_fields = context
        .target_deck
        .resolved_field_graph()
        .expect("selected Workbench target deck was graph-validated when its context was built");
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
                .map(|note| resolved_field_text_or_diagnostic(&target_fields, &note.id, &row.field_id))
                .unwrap_or_else(|| row.translated.clone())
        },
        "status": row.category.as_str(),
        "note_id": row.note_id.to_string(),
        "note_title": note_title(note, source_note_type),
        "field_id": row.field_id.to_string(),
        "field_name": row.field_name,
        "friendly_context": format!("{} · {}", note_title(note, source_note_type), row.field_name),
        "context_path": contextual_path_for_row(row, &context.report.entries),
        "content_group_badges": content_group_badges_for_note(&context.source_deck, &row.note_id),
        "direct_recommended": true,
        "controls": ["direct", "contextual", "no_change"],
        "source_preview": note.and_then(|note| source_note_type.map(|note_type| render_note_cards(&context.source_deck, note, note_type))),
        "target_preview": target_note.and_then(|note| target_note_type.map(|note_type| render_note_cards(&context.target_deck, note, note_type))),
    })
}

fn note_title(note: Option<&Note>, note_type: Option<&NoteType>) -> String {
    let Some(note) = note else {
        return "unknown note".to_owned();
    };
    if let Some(note_type) = note_type {
        for field in &note_type.fields {
            if let Some(value) = note
                .fields
                .get(&field.id)
                .and_then(FieldValue::as_scalar)
                .and_then(title_candidate)
            {
                return value;
            }
        }
    }
    note.fields
        .values()
        .filter_map(FieldValue::as_scalar)
        .find_map(title_candidate)
        .unwrap_or_else(|| note.id.to_string())
}

fn title_candidate(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.trim_start().starts_with('<') {
        return None;
    }
    Some(value.to_owned())
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
            | TranslationCoverageCategory::ContextualTranslation
            | TranslationCoverageCategory::NoChange
            | TranslationCoverageCategory::StaleTranslation
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

fn metadata_category_json(category: &MetadataCategory) -> Value {
    json!({
        "key": category.key,
        "label": category.label,
        "paths": category.paths,
    })
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

fn resolved_field_text_or_diagnostic(
    graph: &brain_brew_core::ResolvedFieldGraph,
    note_id: &StableId,
    field_id: &StableId,
) -> String {
    let path = brain_brew_core::DeckPath::NoteField {
        note_id: note_id.clone(),
        field_id: field_id.clone(),
    }
    .to_string();
    graph
        .get(&path)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("[field resolution error: no live value at {path}]"))
}

fn main_field_rows(
    source_deck: &CanonicalDeck,
    selection: &WorkbenchSelection,
    entries: &[TranslationCoverageEntry],
) -> Vec<MainFieldRow> {
    let structural_fields = structural_field_set(selection, source_deck);
    let resolved_fields = source_deck
        .resolved_field_graph()
        .expect("selected Workbench source deck was graph-validated when its context was built");
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
            let path = format!("notes.{note_id}.fields.{}", field.id);
            let Some(source) = resolved_fields.get(&path).map(str::to_owned) else {
                continue;
            };
            if source.is_empty() {
                continue;
            }
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
                    | TranslationCoverageCategory::ContextualTranslation
                    | TranslationCoverageCategory::NoChange
                    | TranslationCoverageCategory::StaleTranslation
            )
        })
        .count();
    let missing = rows
        .iter()
        .filter(|row| row.category == TranslationCoverageCategory::UntranslatedFallback)
        .count();
    let stale = rows
        .iter()
        .filter(|row| is_stale_category(row.category))
        .count();
    let percent = progress_percent(complete, total);
    json!({
        "complete": complete,
        "total": total,
        "missing": missing,
        "stale": stale,
        "needs_work": missing + stale,
        "percent": percent,
    })
}

fn progress_percent(complete: usize, total: usize) -> usize {
    complete
        .checked_mul(100)
        .and_then(|value| value.checked_div(total))
        .unwrap_or(100)
}

#[derive(Clone, Debug)]
struct OptionalMetadataRow {
    path: String,
    source: String,
    translated: String,
    category: TranslationCoverageCategory,
    metadata_category: String,
    metadata_category_key: String,
    profile_metadata: bool,
    warning: Option<String>,
}

fn optional_metadata_rows(
    context: &SelectedTranslationContext,
    manifest: &FederatedDeckManifest,
) -> Vec<OptionalMetadataRow> {
    let main_paths = main_field_rows(
        &context.source_deck,
        &context.selection,
        &context.report.entries,
    )
    .into_iter()
    .filter(|row| !row.structural)
    .map(|row| row.path)
    .collect::<BTreeSet<_>>();
    let mut rows = context
        .report
        .entries
        .iter()
        .filter(|entry| !main_paths.contains(entry.path.as_str()))
        .filter(|entry| {
            !path_matches_any(
                &manifest.translation_profile.metadata_exclude_paths,
                &entry.path,
            )
        })
        .filter_map(|entry| {
            let (metadata_category_key, metadata_category) = metadata_category_for_path(
                &manifest.translation_profile.metadata_categories,
                &entry.path,
            )?;
            let profile_metadata =
                path_matches_any(&manifest.translation_profile.metadata_paths, &entry.path);
            let warning = is_stale_category(entry.category).then(|| match &entry.old_source {
                Some(old) => format!("stale: source changed from {old:?}"),
                None => "stale metadata".to_owned(),
            });
            Some(OptionalMetadataRow {
                path: entry.path.clone(),
                source: entry.source.clone(),
                translated: entry
                    .translated
                    .clone()
                    .unwrap_or_else(|| entry.source.clone()),
                category: entry.category,
                metadata_category: metadata_category.to_owned(),
                metadata_category_key: metadata_category_key.to_owned(),
                profile_metadata,
                warning,
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        optional_metadata_category_rank(
            &left.metadata_category_key,
            &manifest.translation_profile.metadata_category_order,
            &manifest.translation_profile.metadata_categories,
        )
        .cmp(&optional_metadata_category_rank(
            &right.metadata_category_key,
            &manifest.translation_profile.metadata_category_order,
            &manifest.translation_profile.metadata_categories,
        ))
        .then_with(|| left.path.cmp(&right.path))
        .then_with(|| left.source.cmp(&right.source))
    });
    rows
}

fn optional_metadata_progress(rows: &[OptionalMetadataRow]) -> Value {
    let total = rows.len();
    let complete = rows
        .iter()
        .filter(|row| optional_row_status(row) == "complete")
        .count();
    let missing = rows
        .iter()
        .filter(|row| optional_row_status(row) == "missing")
        .count();
    let stale = rows
        .iter()
        .filter(|row| optional_row_status(row) == "stale")
        .count();
    json!({
        "complete": complete,
        "total": total,
        "missing": missing,
        "stale": stale,
        "needs_work": missing + stale,
    })
}

fn optional_metadata_item_json(row: &OptionalMetadataRow) -> Value {
    json!({
        "path": row.path,
        "source": row.source,
        "target": row.translated,
        "status": optional_row_status(row),
        "coverage_category": format!("{:?}", row.category),
        "metadata_category": row.metadata_category,
        "metadata_category_key": row.metadata_category_key,
        "profile_metadata": row.profile_metadata,
        "warning": row.warning,
        "editable": true,
    })
}

fn optional_row_status(row: &OptionalMetadataRow) -> &'static str {
    if is_stale_category(row.category) {
        "stale"
    } else if row.category == TranslationCoverageCategory::UntranslatedFallback {
        "missing"
    } else {
        "complete"
    }
}

fn metadata_category_for_path<'a>(
    categories: &'a [MetadataCategory],
    path: &str,
) -> Option<(&'a str, &'a str)> {
    categories
        .iter()
        .filter_map(|category| {
            category
                .paths
                .iter()
                .filter(|pattern| path_matches_pattern(pattern, path))
                .map(|pattern| pattern.len())
                .max()
                .map(|matched_len| (matched_len, category))
        })
        .max_by_key(|(matched_len, _)| *matched_len)
        .map(|(_, category)| (category.key.as_str(), category.label.as_str()))
}

fn optional_metadata_category_rank(
    category_key: &str,
    configured_order: &[String],
    categories: &[MetadataCategory],
) -> usize {
    if let Some(index) = configured_order
        .iter()
        .position(|ordered| ordered == category_key)
    {
        return index;
    }
    configured_order.len()
        + categories
            .iter()
            .position(|category| category.key == category_key)
            .unwrap_or(categories.len())
}

fn path_matches_any(patterns: &[String], path: &str) -> bool {
    patterns
        .iter()
        .any(|pattern| path_matches_pattern(pattern, path))
}

fn path_matches_pattern(pattern: &str, path: &str) -> bool {
    let pattern = pattern.as_bytes();
    let path = path.as_bytes();
    let (mut pattern_index, mut path_index) = (0, 0);
    let (mut star_index, mut star_path_index) = (None, 0);

    while path_index < path.len() {
        if pattern_index < pattern.len() && pattern[pattern_index] == path[path_index] {
            pattern_index += 1;
            path_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star_index = Some(pattern_index);
            pattern_index += 1;
            star_path_index = path_index;
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            star_path_index += 1;
            path_index = star_path_index;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }

    pattern_index == pattern.len()
}

fn note_navigation_rows(context: &SelectedTranslationContext, filter: Option<&str>) -> Vec<Value> {
    let rows = main_field_rows(
        &context.source_deck,
        &context.selection,
        &context.report.entries,
    );
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
    for (index, (note_id, note)) in context.source_deck.notes.iter().enumerate() {
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
        let title = note_title(
            Some(note),
            context.source_deck.note_types.get(&note.note_type_id),
        );
        let field_count = field_rows.len();
        let translatable_field_count = field_rows.iter().filter(|row| !row.structural).count();
        let missing_count = field_rows
            .iter()
            .filter(|row| {
                !row.structural && row.category == TranslationCoverageCategory::UntranslatedFallback
            })
            .count();
        let stale_count = note_entries
            .iter()
            .filter(|entry| is_stale_category(entry.category))
            .count();
        let complete_count = translatable_field_count.saturating_sub(missing_count + stale_count);
        notes.push(json!({
            "index": index,
            "note_id": note_id.to_string(),
            "note_type_id": note.note_type_id.to_string(),
            "title": title,
            "status": note_status(&field_rows, &note_entries),
            "field_count": field_count,
            "translatable_field_count": translatable_field_count,
            "complete_count": complete_count,
            "missing_count": missing_count,
            "stale_count": stale_count,
            "content_group_badges": content_group_badges_for_note(&context.source_deck, note_id),
        }));
    }
    notes
}

fn source_string_summary_json(
    group: &SourceStringGroup,
    source_counts: &BTreeMap<String, usize>,
) -> Value {
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
    })
}

fn optional_metadata_summary_json(row: &OptionalMetadataRow) -> Value {
    json!({
        "path": row.path,
        "source": row.source,
        "status": optional_row_status(row),
        "coverage_category": format!("{:?}", row.category),
        "metadata_category": row.metadata_category,
        "metadata_category_key": row.metadata_category_key,
        "profile_metadata": row.profile_metadata,
        "warning": row.warning,
        "editable": true,
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
    let target_fields = target_deck
        .resolved_field_graph()
        .expect("selected Workbench target deck was graph-validated when its context was built");
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
                let target =
                    resolved_field_text_or_diagnostic(&target_fields, note_id, &row.field_id);
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
        let title = note_title(Some(note), Some(note_type));
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
            | TranslationCoverageCategory::StaleTranslation
            | TranslationCoverageCategory::StaleNoChangeKey
            | TranslationCoverageCategory::StaleTargetAdaptation
            | TranslationCoverageCategory::StaleVariableKey
            | TranslationCoverageCategory::StaleAdapterIdKey
            | TranslationCoverageCategory::InvalidTargetAdaptation
    )
}

fn render_note_cards(deck: &CanonicalDeck, note: &Note, note_type: &NoteType) -> Value {
    let rendered_deck = match deck.render_variables() {
        Ok(rendered) => rendered,
        Err(error) => return json!({ "error": error.to_string(), "cards": [] }),
    };
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
        let value = note
            .fields
            .get(&field.id)
            .and_then(FieldValue::as_scalar)
            .unwrap_or("");
        rendered = render_conditional_sections(&rendered, &field.name, value);
    }
    for field in &note_type.fields {
        let value = note
            .fields
            .get(&field.id)
            .and_then(FieldValue::as_scalar)
            .unwrap_or("");
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

fn safe_media_relative_path(path: &str) -> WorkbenchCoreResult<String> {
    SafeRelativePath::new(path)
        .map(|path| path.as_str().to_owned())
        .map_err(|reason| WorkbenchError::request(format!("invalid media path {path:?}: {reason}")))
}

fn missing_media_placeholder_svg(path: &str) -> String {
    let label = html_attribute_escape(path);
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 360 120\" role=\"img\" aria-label=\"missing media {label}\"><rect width=\"360\" height=\"120\" fill=\"#111827\" stroke=\"#3b4a63\"/><text x=\"20\" y=\"52\" fill=\"#f87171\" font-family=\"sans-serif\" font-size=\"18\">Missing media asset</text><text x=\"20\" y=\"82\" fill=\"#bac6d6\" font-family=\"monospace\" font-size=\"14\">{label}</text></svg>"
    )
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

fn read_overlay_document(path: &Path) -> WorkbenchCoreResult<OverlaySourceDocument> {
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    overlay_source_document(path, &input).map_err(WorkbenchError::adapter)
}

fn ensure_root_source_mutable(
    operation: &str,
    source: &PlannedSourceProvenance,
) -> WorkbenchCoreResult<()> {
    if source.registry_source == RegistrySourceKind::RootManifest {
        Ok(())
    } else {
        Err(WorkbenchError::new(
            WorkbenchErrorKind::ReadOnly,
            format!(
                "{operation} is read-only for {} source {} at {}; dependency/include/locked/cache sources cannot be selected for Workbench mutation",
                source.registry_source.ownership_name(),
                match source.kind {
                    crate::planner::PlanSourceKind::Base => "base",
                    crate::planner::PlanSourceKind::Overlay { .. } => "overlay",
                    crate::planner::PlanSourceKind::ScalarInclude { .. } => "scalar include",
                    crate::planner::PlanSourceKind::MediaInclude => "media include",
                },
                source.path.display()
            ),
        ))
    }
}

fn planned_overlay_for_path<'a>(
    context: &'a SelectedTranslationContext,
    path: &Path,
) -> WorkbenchCoreResult<&'a PlannedOverlay> {
    context
        .plan_overlays
        .iter()
        .map(|(planned, _)| planned)
        .find(|planned| planned.file == path)
        .ok_or_else(|| {
            WorkbenchError::not_found(format!("no planned overlay owns {}", path.display()))
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum OriginalSourceSnapshot {
    Present(Vec<u8>),
    Absent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PlannedSourceOutput {
    original: OriginalSourceSnapshot,
    replacement: Vec<u8>,
}

#[derive(Default)]
struct SourceApplyPlan {
    changed_entries: Vec<Value>,
    overlay_changed: bool,
    outputs: BTreeMap<PathBuf, PlannedSourceOutput>,
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
    overlay_document: &mut OverlaySourceDocument,
    edits: &[StagedWorkbenchEdit],
    context: &SelectedTranslationContext,
    base_file: &Path,
    manifest_root: &Path,
) -> WorkbenchCoreResult<SourceApplyPlan> {
    let mut plan = SourceApplyPlan::default();
    if edits.is_empty() {
        return Ok(plan);
    }

    ensure_root_source_mutable("Workbench source edit", &context.base_source)?;
    for include in &context.base_includes {
        ensure_root_source_mutable("Workbench included source edit", include)?;
    }
    let raw_deck_yaml = fs::read_to_string(base_file)
        .map_err(|error| format!("{}: {error}", base_file.display()))?;
    let mut document = canonical_source_document(base_file, &raw_deck_yaml)?;
    let rows = main_field_rows(
        &context.source_deck,
        &context.selection,
        &context.report.entries,
    );
    let row_by_path = rows
        .iter()
        .map(|row| (row.path.clone(), row.clone()))
        .collect::<BTreeMap<_, _>>();

    for edit in edits {
        if !edit.path.starts_with("notes.") || !edit.path.contains(".fields.") {
            return Err(WorkbenchError::request(format!(
                "invalid source note-field edit path {:?}",
                edit.path
            )));
        }
        if edit.source.is_empty() {
            return Err(WorkbenchError::request(format!(
                "invalid empty source for {}",
                edit.path
            )));
        }
        let Some(anchor_row) = row_by_path.get(&edit.path) else {
            return Err(WorkbenchError::request(format!(
                "invalid source edit path {:?}; choose an editable source note field",
                edit.path
            )));
        };
        if anchor_row.source != edit.source {
            return Err(WorkbenchError::request(format!(
                "invalid source {:?} for {}; expected {:?}",
                edit.source, edit.path, anchor_row.source
            )));
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
            let (note_id, field_id) = note_field_path(&row.path)?;
            let location = document
                .set_scalar(
                    CanonicalScalarTarget::NoteField {
                        note_id: StableId::new(note_id).map_err(|error| error.to_string())?,
                        field_id: StableId::new(field_id).map_err(|error| error.to_string())?,
                    },
                    &edit.source,
                    &edit.value,
                )
                .map_err(|error| error.to_string())?;
            set_deck_note_field(modified_base, &row.path, &edit.source, &edit.value)?;
            let changed_path = match location {
                EditLocation::Root => base_file.to_path_buf(),
                EditLocation::Included(provenance) => PathBuf::from(provenance.source_name()),
            };
            plan.affected_files.insert(
                changed_path.clone(),
                workspace_path(manifest_root, &changed_path),
            );
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
            overlay_document,
            edit,
            &target_rows,
            old_source_remains,
            context,
            &mut plan.changed_entries,
        )?;
    }

    collect_document_emission(
        document.emit().map_err(|error| error.to_string())?,
        &mut plan.outputs,
    )?;
    plan.outputs
        .retain(|path, _| plan.affected_files.contains_key(path));
    Ok(plan)
}

fn apply_source_translation_impact(
    overlay: &mut Overlay,
    document: &mut OverlaySourceDocument,
    edit: &StagedWorkbenchEdit,
    target_rows: &[MainFieldRow],
    old_source_remains: bool,
    context: &SelectedTranslationContext,
    changed_entries: &mut Vec<Value>,
) -> WorkbenchCoreResult<bool> {
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
                    | TranslationCoverageCategory::StaleTranslation
            )
        })
        && target_rows.iter().all(|row| {
            row.category != TranslationCoverageCategory::StaleTranslation
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
    for row in impact_rows {
        let Some(target) = target_text_for_source_impact(&row) else {
            continue;
        };
        let context_path = if use_global_impact {
            None
        } else {
            Some(contextual_path_for_row(&row, &context.report.entries))
        };
        let impact = match edit.impact_action {
            SourceImpactAction::StaleTranslation => SourceTranslationImpact::MarkStale {
                target: target.clone(),
                context: context_path.clone(),
            },
            SourceImpactAction::MigrateKey => SourceTranslationImpact::MigrateKey {
                target: target.clone(),
                context: context_path.clone(),
            },
        };
        document
            .apply_source_translation_impact(&row.path, &edit.source, &edit.value, impact)
            .map_err(|error| error.to_string())?;
        changed_entries.push(json!({
            "mode": match edit.impact_action {
                SourceImpactAction::StaleTranslation => "stale_translation",
                SourceImpactAction::MigrateKey => "migrate_key",
            },
            "path": row.path,
            "old_source": edit.source,
            "new_source": edit.value,
            "target": target,
            "context": context_path,
            "impact_action": match edit.impact_action {
                SourceImpactAction::StaleTranslation => "stale_translation",
                SourceImpactAction::MigrateKey => "migrate_key",
            },
            "available_impact_actions": ["stale_translation", "migrate_key"],
        }));
        changed = true;
    }
    if changed {
        *overlay = document.resolved_overlay().clone();
    }
    Ok(changed)
}

fn target_text_for_source_impact(row: &MainFieldRow) -> Option<String> {
    match row.category {
        TranslationCoverageCategory::DirectTranslation
        | TranslationCoverageCategory::ContextualTranslation
        | TranslationCoverageCategory::NoChange
        | TranslationCoverageCategory::StaleTranslation => Some(row.translated.clone()),
        _ => None,
    }
}

fn set_deck_note_field(
    deck: &mut CanonicalDeck,
    path: &str,
    expected_source: &str,
    value: &str,
) -> WorkbenchCoreResult<()> {
    let (note_id, field_id) = note_field_path(path)?;
    let note_id =
        StableId::new(note_id).map_err(|error| WorkbenchError::request(error.to_string()))?;
    let field_id =
        StableId::new(field_id).map_err(|error| WorkbenchError::request(error.to_string()))?;
    let note = deck.notes.get_mut(&note_id).ok_or_else(|| {
        WorkbenchError::request(format!(
            "source edit path {path:?} is not in the canonical deck file"
        ))
    })?;
    let current = note
        .fields
        .get(&field_id)
        .and_then(FieldValue::as_scalar)
        .unwrap_or_default();
    if current != expected_source {
        return Err(WorkbenchError::request(format!(
            "invalid source {:?} for {}; expected canonical deck value {:?}",
            expected_source, path, current
        )));
    }
    note.fields
        .insert(field_id, FieldValue::Scalar(value.to_owned()));
    Ok(())
}

fn note_field_path(path: &str) -> WorkbenchCoreResult<(&str, &str)> {
    let Some(rest) = path.strip_prefix("notes.") else {
        return Err((format!("invalid note-field path {path:?}")).into());
    };
    let Some((note_id, field_id)) = rest.split_once(".fields.") else {
        return Err((format!("invalid note-field path {path:?}")).into());
    };
    if note_id.is_empty() || field_id.is_empty() {
        return Err((format!("invalid note-field path {path:?}")).into());
    }
    Ok((note_id, field_id))
}

fn collect_document_emission(
    emission: SourceDocumentEmission,
    outputs: &mut BTreeMap<PathBuf, PlannedSourceOutput>,
) -> WorkbenchCoreResult<()> {
    collect_document_source(
        emission.root(),
        emission.original_source(emission.root().provenance()),
        outputs,
    )?;
    for source in emission.included() {
        collect_document_source(
            source,
            emission.original_source(source.provenance()),
            outputs,
        )?;
    }
    Ok(())
}

fn collect_document_source(
    source: &brain_brew_formats::source_document::SourceFile,
    original: Option<&brain_brew_formats::source_document::SourceFile>,
    outputs: &mut BTreeMap<PathBuf, PlannedSourceOutput>,
) -> WorkbenchCoreResult<()> {
    let path = PathBuf::from(source.provenance().source_name());
    let output = PlannedSourceOutput {
        original: original.map_or(OriginalSourceSnapshot::Absent, |original| {
            OriginalSourceSnapshot::Present(original.text().as_bytes().to_vec())
        }),
        replacement: source.text().as_bytes().to_vec(),
    };
    if let Some(previous) = outputs.insert(path.clone(), output.clone())
        && previous != output
    {
        return Err((format!(
            "conflicting Workbench outputs or original snapshots for {}",
            path.display()
        ))
        .into());
    }
    Ok(())
}

fn planned_workbench_outputs(
    workspace_root: &Path,
    outputs: BTreeMap<PathBuf, PlannedSourceOutput>,
) -> WorkbenchCoreResult<Vec<PlannedWorkspaceFile>> {
    let canonical_root = fs::canonicalize(workspace_root)
        .map_err(|error| format!("{}: {error}", workspace_root.display()))?;
    outputs
        .into_iter()
        .map(|(path, output)| {
            let absolute = if path.is_absolute() {
                path
            } else {
                canonical_root.join(path)
            };
            let canonical = match &output.original {
                OriginalSourceSnapshot::Present(_) => {
                    let parent = absolute
                        .parent()
                        .ok_or_else(|| format!("{} has no parent", absolute.display()))?;
                    let canonical_parent = fs::canonicalize(parent)
                        .map_err(|error| format!("{}: {error}", parent.display()))?;
                    let name = absolute
                        .file_name()
                        .ok_or_else(|| format!("{} has no file name", absolute.display()))?;
                    canonical_parent.join(name)
                }
                OriginalSourceSnapshot::Absent => {
                    let mut ancestor = absolute.as_path();
                    let mut suffix = Vec::new();
                    while !ancestor.exists() {
                        suffix.push(
                            ancestor
                                .file_name()
                                .ok_or_else(|| {
                                    format!("{} has no existing ancestor", absolute.display())
                                })?
                                .to_os_string(),
                        );
                        ancestor = ancestor.parent().ok_or_else(|| {
                            format!("{} has no existing ancestor", absolute.display())
                        })?;
                    }
                    let mut resolved = fs::canonicalize(ancestor)
                        .map_err(|error| format!("{}: {error}", ancestor.display()))?;
                    for component in suffix.iter().rev() {
                        resolved.push(component);
                    }
                    resolved
                }
            };
            if !canonical.starts_with(&canonical_root) {
                return Err(format!(
                    "Workbench mutation target {} is outside root workspace {}",
                    canonical.display(),
                    canonical_root.display()
                ));
            }
            match output.original {
                OriginalSourceSnapshot::Present(original) => PlannedWorkspaceFile::validated(
                    canonical,
                    original,
                    output.replacement,
                    validate_utf8_workbench_output,
                ),
                OriginalSourceSnapshot::Absent => PlannedWorkspaceFile::validated_new(
                    canonical,
                    output.replacement,
                    validate_utf8_workbench_output,
                ),
            }
        })
        .collect::<Result<Vec<_>, String>>()
        .map_err(WorkbenchError::adapter)
}

fn validate_utf8_workbench_output(bytes: &[u8]) -> Result<(), String> {
    std::str::from_utf8(bytes)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn content_diagnostics(report: &ContentValidationReport) -> Vec<DomainDiagnostic> {
    report
        .errors
        .iter()
        .map(|error| DomainDiagnostic {
            code: match error.kind {
                ContentKind::HtmlFragment => "invalid_html_content",
                ContentKind::Css => "invalid_css_content",
            },
            category: DiagnosticCategory::Validation,
            path: error.path.parse().ok(),
            address: error.path.clone(),
            overlay_id: None,
            source_id: None,
            intent: None,
            entity_kind: None,
            expected: None,
            actual: None,
            first_conflict_participant: None,
            current_conflict_participant: None,
            original_removal: None,
            field_graph_error: None,
            children: Vec::new(),
            message: match error.line {
                Some(line) => format!("line {line}: {}", error.message),
                None => error.message.clone(),
            },
        })
        .collect()
}

fn validate_complete_workbench_result(
    modified_base: &CanonicalDeck,
    overlays: &BTreeMap<PathBuf, (String, OverlaySourceDocument, Overlay, bool)>,
    contexts: &[SelectedTranslationContext],
) -> WorkbenchCoreResult<Vec<DomainDiagnostic>> {
    canonical_yaml::to_string(modified_base).map_err(|error| error.to_string())?;
    if !modified_base.media.is_empty() {
        media::validate_references(modified_base).map_err(|error| error.to_string())?;
    }
    for (display, document, overlay, changed) in overlays.values() {
        if !changed {
            continue;
        }
        document
            .emit()
            .map_err(|error| format!("invalid generated translation overlay {display}: {error}"))?;
        canonical_yaml::overlay_to_string(overlay)
            .map_err(|error| format!("invalid generated translation overlay {display}: {error}"))?;
    }
    let mut diagnostics = Vec::new();
    let mut validated = BTreeSet::new();
    for context in contexts {
        if !validated.insert(context.selection.target_id.clone()) {
            continue;
        }
        let mut deck = modified_base.clone();
        for (planned, original) in &context.plan_overlays {
            let overlay = overlays
                .get(&planned.file)
                .filter(|(_, _, _, changed)| *changed)
                .map(|(_, _, overlay, _)| overlay)
                .unwrap_or(original);
            let effective_overlay = if overlay.translations.is_some() {
                sanitize_lenient_translation_overlay(&deck, overlay).map_err(|report| {
                    WorkbenchError::field_graph(
                        format!("failed to validate translation overlay {}", planned.id),
                        report,
                    )
                })?
            } else {
                overlay.clone()
            };
            deck = deck
                .compose(std::slice::from_ref(&effective_overlay))
                .map_err(|report| {
                    WorkbenchError::compose(
                        format!(
                            "failed to validate complete Workbench target {} at overlay {}",
                            context.selection.target_id, planned.id
                        ),
                        report,
                    )
                })?;
        }
        let rendered = deck.render_variables().map_err(|report| {
            WorkbenchError::render(
                format!(
                    "failed to render complete Workbench target {}",
                    context.selection.target_id
                ),
                report,
            )
        })?;
        diagnostics.extend(content_diagnostics(&validate_deck_content(&rendered)));
        if !deck.media.is_empty() {
            media::validate_references(&deck).map_err(|error| {
                format!(
                    "media validation failed for complete Workbench target {}: {error}",
                    context.selection.target_id
                )
            })?;
        }
    }
    Ok(diagnostics)
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
    for entry in &context.report.entries {
        if entry.source.is_empty()
            || path_matches_any(&context.selection.metadata_exclude_paths, &entry.path)
            || metadata_category_for_path(&context.selection.metadata_categories, &entry.path)
                .is_none()
        {
            continue;
        }
        sources
            .entry(entry.path.clone())
            .or_insert_with(|| entry.source.clone());
    }
    sources
}

fn contextual_path_for_edit(
    context: &SelectedTranslationContext,
    edit: &StagedWorkbenchEdit,
) -> WorkbenchCoreResult<String> {
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
        .find(|row| row.path == edit.path && row.source == edit.source && !row.structural);
    if let Some(row) = row {
        if let Some(context_path) = &edit.context_path {
            if context_path == &row.path
                || row
                    .path
                    .strip_prefix(context_path)
                    .is_some_and(|suffix| suffix.starts_with('.'))
            {
                return Ok(context_path.clone());
            }
            return Err((format!(
                "invalid contextual edit context {:?} for {}",
                context_path, edit.path
            ))
            .into());
        }
        return Ok(contextual_path_for_row(row, &context.report.entries));
    }

    if !path_matches_any(&context.selection.metadata_exclude_paths, &edit.path)
        && metadata_category_for_path(&context.selection.metadata_categories, &edit.path).is_some()
        && context
            .report
            .entries
            .iter()
            .any(|entry| entry.path == edit.path && entry.source == edit.source)
    {
        if let Some(context_path) = &edit.context_path {
            if context_path == &edit.path
                || edit
                    .path
                    .strip_prefix(context_path)
                    .is_some_and(|suffix| suffix.starts_with('.'))
            {
                return Ok(context_path.clone());
            }
            return Err((format!(
                "invalid contextual edit context {:?} for {}",
                context_path, edit.path
            ))
            .into());
        }
        return Ok(edit.path.clone());
    }

    Err(WorkbenchError::request(format!(
        "invalid contextual edit path {:?}",
        edit.path
    )))
}

fn apply_staged_edits_to_overlay(
    overlay: &mut Overlay,
    document: &mut OverlaySourceDocument,
    edits: &[StagedWorkbenchEdit],
    context: &SelectedTranslationContext,
) -> WorkbenchCoreResult<Vec<Value>> {
    let editable_sources = editable_sources_by_path(context);
    let mut changed = Vec::new();
    for edit in edits {
        if edit.source.is_empty() {
            return Err(WorkbenchError::request(format!(
                "invalid empty source for {}",
                edit.path
            )));
        }
        let Some(expected_source) = editable_sources.get(edit.path.as_str()) else {
            return Err(WorkbenchError::request(format!(
                "invalid edit path {:?}; choose an editable translation source in the selected target",
                edit.path
            )));
        };
        if expected_source != &edit.source {
            return Err(WorkbenchError::request(format!(
                "invalid source {:?} for {}; expected {:?}",
                edit.source, edit.path, expected_source
            )));
        }
        let translations = overlay.translations.as_ref();
        match edit.mode {
            EditMode::Direct => {
                let old = translations
                    .and_then(|dictionary| dictionary.direct.get(&edit.source))
                    .cloned();
                document
                    .set_translation_decision(
                        &edit.path,
                        &edit.source,
                        TranslationDecision::Direct(edit.value.clone()),
                    )
                    .map_err(|error| error.to_string())?;
                changed.push(json!({
                    "mode": "direct",
                    "path": edit.path,
                    "source": edit.source,
                    "old": old,
                    "new": edit.value,
                }));
            }
            EditMode::Contextual => {
                let context_path = contextual_path_for_edit(context, edit)?;
                let old = translations
                    .and_then(|dictionary| dictionary.contextual.get(&context_path))
                    .and_then(|replacements| replacements.get(&edit.source))
                    .cloned();
                document
                    .set_translation_decision(
                        &edit.path,
                        &edit.source,
                        TranslationDecision::Contextual {
                            context: context_path.clone(),
                            target: edit.value.clone(),
                        },
                    )
                    .map_err(|error| error.to_string())?;
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
                let existed = translations
                    .is_some_and(|dictionary| dictionary.no_change.contains(&edit.source));
                document
                    .set_translation_decision(
                        &edit.path,
                        &edit.source,
                        TranslationDecision::NoChange,
                    )
                    .map_err(|error| error.to_string())?;
                changed.push(json!({
                    "mode": "no_change",
                    "path": edit.path,
                    "source": edit.source,
                    "old": if existed { json!(edit.source) } else { Value::Null },
                    "new": edit.source,
                }));
            }
        }
        *overlay = document.resolved_overlay().clone();
    }
    Ok(changed)
}

fn validate_modified_base_and_overlay(
    context: &SelectedTranslationContext,
    modified_base: &CanonicalDeck,
    modified_overlay: &Overlay,
) -> WorkbenchCoreResult<ApplyValidation> {
    let updated_context =
        context_with_modified_base_and_overlay(context, modified_base.clone(), modified_overlay)?;
    let rendered = updated_context
        .target_deck
        .render_variables()
        .map_err(|report| {
            WorkbenchError::render(
                format!(
                    "failed to render modified Workbench target {}",
                    updated_context.selection.target_id
                ),
                report,
            )
        })?;
    Ok(ApplyValidation {
        diagnostics: content_diagnostics(&validate_deck_content(&rendered)),
    })
}

fn context_with_modified_base_and_overlay(
    context: &SelectedTranslationContext,
    modified_base: CanonicalDeck,
    modified_overlay: &Overlay,
) -> WorkbenchCoreResult<SelectedTranslationContext> {
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
            selected_report = Some(current.translation_coverage(active_overlay).map_err(
                |report| {
                    WorkbenchError::field_graph(
                        "failed to resolve translation source fields",
                        report,
                    )
                },
            )?);
        }
        let effective_overlay = if active_overlay.translations.is_some() {
            sanitize_lenient_translation_overlay(&current, active_overlay).map_err(|report| {
                WorkbenchError::field_graph(
                    format!("failed to resolve translation overlay {}", planned.id),
                    report,
                )
            })?
        } else {
            active_overlay.clone()
        };
        current = current
            .compose(std::slice::from_ref(&effective_overlay))
            .map_err(|report| {
                WorkbenchError::compose(format!("failed to compose overlay {}", planned.id), report)
            })?;
    }
    let Some(source_deck) = selected_source_deck else {
        return Err((format!(
            "target {} does not include translation overlay {}",
            context.selection.target_id, context.selection.overlay_id
        ))
        .into());
    };
    let Some(report) = selected_report else {
        return Err((format!(
            "overlay {} is not a translation overlay",
            context.selection.overlay_id
        ))
        .into());
    };
    Ok(SelectedTranslationContext {
        selection: context.selection.clone(),
        base_deck: modified_base,
        base_source: context.base_source.clone(),
        base_includes: context.base_includes.clone(),
        plan_overlays: context.plan_overlays.clone(),
        media_declarations: context.media_declarations.clone(),
        source_deck,
        target_deck: current,
        report,
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
    _manifest: &FederatedDeckManifest,
) -> WorkbenchCoreResult<Vec<Value>> {
    let registry = ManifestRegistry::load(manifest_path, &[], &[])?;
    let mut entries = Vec::new();
    for loaded in registry.manifests() {
        entries.push(file_fingerprint(
            root,
            &loaded.path,
            loaded
                .identity
                .as_ref()
                .map(|identity| identity.id.as_str()),
            &loaded.root,
            loaded.discovery.ownership_name(),
        )?);
    }
    for source in registry.registered_sources()? {
        entries.push(file_fingerprint(
            root,
            &source.path,
            source.package.as_ref().map(|identity| identity.id.as_str()),
            &source.package_root,
            source.registry_source.ownership_name(),
        )?);
    }
    entries.sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
    entries.dedup_by(|left, right| left["path"] == right["path"]);
    Ok(entries)
}

fn authorized_manifest_source_files(
    manifest_path: &Path,
    _root: &Path,
    _manifest: &FederatedDeckManifest,
) -> WorkbenchCoreResult<Vec<PathBuf>> {
    let registry = ManifestRegistry::load(manifest_path, &[], &[])?;
    let mut files = registry
        .manifests()
        .iter()
        .map(|loaded| loaded.path.clone())
        .collect::<Vec<_>>();
    files.extend(
        registry
            .registered_sources()?
            .into_iter()
            .map(|source| source.path),
    );
    files.sort();
    files.dedup();
    Ok(files)
}

fn file_fingerprint(
    root: &Path,
    path: &Path,
    package: Option<&str>,
    package_root: &Path,
    source_kind: &str,
) -> WorkbenchCoreResult<Value> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let hash = Sha256::digest(&bytes);
    Ok(json!({
        "path": workspace_path(root, path),
        "sha256": format!("{hash:x}"),
        "package": package,
        "package_root": package_root.display().to_string(),
        "source_kind": source_kind,
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

#[cfg(test)]
mod transaction_snapshot_tests {
    use super::*;

    fn assert_t0_conflict(
        root: &Path,
        target: &Path,
        outputs: BTreeMap<PathBuf, PlannedSourceOutput>,
        external: &[u8],
    ) {
        fs::write(target, external).unwrap();
        let writes = planned_workbench_outputs(root, outputs).unwrap();
        let error = commit_workspace_files(root, writes).unwrap_err();
        assert!(
            error.contains("does not match expected state"),
            "unexpected conflict: {error}"
        );
        assert_eq!(fs::read(target).unwrap(), external);
    }

    #[test]
    fn canonical_source_snapshot_is_not_rebased_during_transaction_planning() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("deck.yaml");
        let original = include_str!("../../../../fixtures/ug-style/deck.yaml");
        fs::write(&path, original).unwrap();
        let mut document = canonical_source_document(&path, original).unwrap();
        document
            .set_scalar(
                CanonicalScalarTarget::NoteField {
                    note_id: StableId::new("note.finland").unwrap(),
                    field_id: StableId::new("field.country").unwrap(),
                },
                "Finland",
                "Computed replacement",
            )
            .unwrap();
        let mut outputs = BTreeMap::new();
        collect_document_emission(document.emit().unwrap(), &mut outputs).unwrap();

        assert_t0_conflict(
            workspace.path(),
            &path,
            outputs,
            b"external canonical edit\n",
        );
    }

    #[test]
    fn included_source_snapshot_uses_exact_loader_bytes() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("deck.yaml");
        let include = workspace.path().join("description.md");
        let original = "deck:\n  id: deck.snapshot\n  name: Snapshot\n  description: !include description.md\n  adapter_ids: {}\nnote_types: {}\nnotes: {}\nmedia: {}\ntombstones: []\n";
        fs::write(&path, original).unwrap();
        fs::write(&include, "loader bytes\n").unwrap();
        let mut document = canonical_source_document(&path, original).unwrap();
        document
            .set_scalar(
                CanonicalScalarTarget::DeckDescription,
                "loader bytes\n",
                "computed include replacement\n",
            )
            .unwrap();
        let mut outputs = BTreeMap::new();
        collect_document_emission(document.emit().unwrap(), &mut outputs).unwrap();
        outputs.retain(|output_path, _| output_path == &include);

        assert_t0_conflict(
            workspace.path(),
            &include,
            outputs,
            b"external include edit\n",
        );
    }

    #[test]
    fn translation_overlay_snapshot_is_not_rebased_during_transaction_planning() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("da.yaml");
        let original = "id: overlay.translation.da\nkind: translation\ntranslations:\n  direct:\n    Finland: Finlande\n";
        fs::write(&path, original).unwrap();
        let mut document = overlay_source_document(&path, original).unwrap();
        document
            .set_translation_decision(
                "notes.note.finland.fields.field.country",
                "Finland",
                TranslationDecision::Direct("Suomi".to_owned()),
            )
            .unwrap();
        let mut outputs = BTreeMap::new();
        collect_document_emission(document.emit().unwrap(), &mut outputs).unwrap();

        assert_t0_conflict(
            workspace.path(),
            &path,
            outputs,
            b"external translation edit\n",
        );
    }

    #[test]
    fn new_language_manifest_and_source_expectations_are_not_rebased() {
        let manifest_workspace = tempfile::tempdir().unwrap();
        let manifest_path = manifest_workspace.path().join("brainbrew.yaml");
        let original_manifest = b"package: old\n".to_vec();
        fs::write(&manifest_path, &original_manifest).unwrap();
        let manifest_outputs = BTreeMap::from([(
            manifest_path.clone(),
            PlannedSourceOutput {
                original: OriginalSourceSnapshot::Present(original_manifest),
                replacement: b"package: computed\n".to_vec(),
            },
        )]);
        assert_t0_conflict(
            manifest_workspace.path(),
            &manifest_path,
            manifest_outputs,
            b"package: external\n",
        );

        let source_workspace = tempfile::tempdir().unwrap();
        let source_path = source_workspace.path().join("overlays/new.yaml");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        let source_outputs = BTreeMap::from([(
            source_path.clone(),
            PlannedSourceOutput {
                original: OriginalSourceSnapshot::Absent,
                replacement: b"id: overlay.translation.new\nkind: translation\n".to_vec(),
            },
        )]);
        assert_t0_conflict(
            source_workspace.path(),
            &source_path,
            source_outputs,
            b"external new-language source\n",
        );
    }
}
