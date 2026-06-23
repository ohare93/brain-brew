use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::get;
use axum::{Json, Router};
use brain_brew_formats::manifest::{FederatedDeckManifest, LanguageManifestEntry};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};

use crate::help;
use crate::io::{manifest_root, read_manifest};

const EMBEDDED_WORKBENCH_INDEX: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Brain Brew Deck Workbench</title>
  </head>
  <body>
    <main id="brainbrew-workbench">
      <h1>Brain Brew Deck Workbench</h1>
      <p>The embedded workbench shell is installed. Build the WASM UI or use --dev-assets for a development shell.</p>
    </main>
  </body>
</html>
"#;

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
        .with_state(metadata);

    if let Some(assets) = dev_assets {
        router.fallback_service(
            ServeDir::new(&assets).not_found_service(ServeFile::new(assets.join("index.html"))),
        )
    } else {
        router
            .route("/", get(embedded_index))
            .fallback(get(embedded_index))
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

async fn embedded_index() -> Html<&'static str> {
    Html(EMBEDDED_WORKBENCH_INDEX)
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
