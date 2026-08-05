use std::fs;
use std::path::{Path, PathBuf};

use brain_brew_core::{CanonicalDeck, Overlay};
use brain_brew_formats::canonical_source_document::CanonicalSourceDocument;
use brain_brew_formats::csv_note_source::{CsvSourceFile, CsvSourceRequest};
use brain_brew_formats::overlay_source_document::OverlaySourceDocument;
use brain_brew_formats::source_document::{IncludeRequest, SourceFile, SourceProvenance};
use brain_brew_formats::{
    canonical_yaml, lockfile, manifest, media_map, note_type_map, source_includes,
};
use serde_yaml::Value;

use crate::path_authorization::PathAuthorizer;

pub(crate) fn format_source(input: &str) -> Result<String, String> {
    let mut errors = Vec::new();
    match source_includes::format_preserving_file_includes(input, canonical_yaml::format_str) {
        Ok(formatted) => return Ok(formatted),
        Err(error) => errors.push(format!("deck: {error}")),
    }
    match source_includes::format_preserving_file_includes(
        input,
        canonical_yaml::overlay_format_str,
    ) {
        Ok(formatted) => return Ok(formatted),
        Err(error) => errors.push(format!("overlay: {error}")),
    }
    match manifest::format_str(input) {
        Ok(formatted) => return Ok(formatted),
        Err(error) => errors.push(format!("manifest: {error}")),
    }
    match lockfile::format_str(input) {
        Ok(formatted) => return Ok(formatted),
        Err(error) => errors.push(format!("lockfile: {error}")),
    }
    match media_map::format_str(input) {
        Ok(formatted) => return Ok(formatted),
        Err(error) => errors.push(format!("media map: {error}")),
    }
    match source_includes::format_preserving_file_includes(input, note_type_map::format_str) {
        Ok(formatted) => return Ok(formatted),
        Err(error) => errors.push(format!("note-type map: {error}")),
    }
    Err(format!(
        "unrecognized Brain Brew source file ({})",
        errors.join("; ")
    ))
}

pub(crate) fn format_source_at(path: &Path, input: &str) -> Result<String, String> {
    match canonical_source_document(path, input) {
        Ok(document) => {
            return document
                .emit()
                .map(|emission| emission.root().text().to_owned())
                .map_err(|error| error.to_string());
        }
        Err(error) if is_canonical_deck_source(input) => return Err(error),
        Err(_) => {}
    }
    if let Ok(document) = overlay_source_document(path, input) {
        return document
            .emit()
            .map(|emission| emission.root().text().to_owned())
            .map_err(|error| error.to_string());
    }
    format_source(input)
}

pub(crate) fn is_canonical_deck_source(input: &str) -> bool {
    let Ok(Value::Mapping(mapping)) = serde_yaml::from_str::<Value>(input) else {
        return false;
    };
    ["deck", "note_types", "notes", "media"]
        .into_iter()
        .all(|expected| mapping.keys().any(|key| key.as_str() == Some(expected)))
}

pub(crate) fn canonical_source_document(
    path: &Path,
    input: &str,
) -> Result<CanonicalSourceDocument, String> {
    let context = source_context_for_path(path)?;
    parse_canonical_source(source_file(path, input, &context)?, &context)
}

pub(crate) fn overlay_source_document(
    path: &Path,
    input: &str,
) -> Result<OverlaySourceDocument, String> {
    let context = source_context_for_path(path)?;
    OverlaySourceDocument::parse_with_csv_inventory(
        source_file(path, input, &context)?,
        |request| load_source_include(request, &context),
        |request| load_csv_source(request, &context),
    )
    .map_err(|error| error.to_string())
}

fn source_file(path: &Path, input: &str, context: &SourceContext) -> Result<SourceFile, String> {
    let absolute =
        fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(SourceFile::new(
        SourceProvenance::new(absolute.display().to_string())
            .with_source_root(context.root.display().to_string()),
        input,
    ))
}

fn parse_canonical_source(
    source: SourceFile,
    context: &SourceContext,
) -> Result<CanonicalSourceDocument, String> {
    CanonicalSourceDocument::parse_with_csv_sources(
        source,
        |request| load_source_include(request, context),
        |request| load_csv_source(request, context),
    )
    .map_err(|error| error.to_string())
}

fn load_source_include(
    request: &IncludeRequest,
    context: &SourceContext,
) -> Result<SourceFile, String> {
    let declaring = Path::new(request.referring_source().source_name());
    let absolute =
        authorize_include_read(declaring, request.schema_path(), request.target(), context)?;
    let text = fs::read_to_string(&absolute)
        .map_err(|error| format!("{}: {error}", absolute.display()))?;
    Ok(SourceFile::new(
        SourceProvenance::new(absolute.display().to_string())
            .with_source_root(context.root.display().to_string()),
        text,
    ))
}

fn load_csv_source(
    request: &CsvSourceRequest,
    context: &SourceContext,
) -> Result<CsvSourceFile, String> {
    let referring = Path::new(request.referring_source().source_name());
    let absolute = PathAuthorizer::new("package", &context.root)?
        .authorize_read_relative_to(referring, request.schema_path(), request.target())
        .map_err(|error| error.to_string())?
        .into_path_buf();
    let bytes = fs::read(&absolute).map_err(|error| format!("{}: {error}", absolute.display()))?;
    Ok(CsvSourceFile::new(
        SourceProvenance::new(absolute.display().to_string())
            .with_source_root(context.root.display().to_string()),
        bytes,
    ))
}

pub(crate) fn read_deck(path: &Path) -> Result<CanonicalDeck, String> {
    let context = source_context_for_path(path)?;
    read_deck_with_context(path, &context)
}

pub(crate) fn canonical_document_from_package(
    path: &Path,
    package_root: &Path,
    include_roots: &[PathBuf],
) -> Result<CanonicalSourceDocument, String> {
    let context = SourceContext {
        root: package_root.to_path_buf(),
        include_roots: include_roots.to_vec(),
    };
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    parse_canonical_source(source_file(path, &input, &context)?, &context)
}

fn read_deck_with_context(path: &Path, context: &SourceContext) -> Result<CanonicalDeck, String> {
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    parse_canonical_source(source_file(path, &input, context)?, context)
        .map(|document| document.resolved_deck().clone())
}

pub(crate) fn overlay_document_from_package(
    path: &Path,
    package_root: &Path,
    include_roots: &[PathBuf],
) -> Result<OverlaySourceDocument, String> {
    overlay_document_from_package_inner(path, package_root, include_roots, None)
}

pub(crate) fn overlay_document_from_package_with_source_deck(
    path: &Path,
    package_root: &Path,
    include_roots: &[PathBuf],
    source_deck: &CanonicalDeck,
) -> Result<OverlaySourceDocument, String> {
    overlay_document_from_package_inner(path, package_root, include_roots, Some(source_deck))
}

fn overlay_document_from_package_inner(
    path: &Path,
    package_root: &Path,
    include_roots: &[PathBuf],
    source_deck: Option<&CanonicalDeck>,
) -> Result<OverlaySourceDocument, String> {
    let context = SourceContext {
        root: package_root.to_path_buf(),
        include_roots: include_roots.to_vec(),
    };
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let source = source_file(path, &input, &context)?;
    let result = if let Some(source_deck) = source_deck {
        OverlaySourceDocument::parse_with_csv_translations(
            source,
            source_deck,
            |request| load_source_include(request, &context),
            |request| load_csv_source(request, &context),
        )
    } else {
        OverlaySourceDocument::parse_with_csv_inventory(
            source,
            |request| load_source_include(request, &context),
            |request| load_csv_source(request, &context),
        )
    };
    result.map_err(|error| error.to_string())
}

fn read_overlay_with_context(
    path: &Path,
    context: &SourceContext,
    source_deck: &CanonicalDeck,
) -> Result<Overlay, String> {
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    OverlaySourceDocument::parse_with_csv_translations(
        source_file(path, &input, context)?,
        source_deck,
        |request| load_source_include(request, context),
        |request| load_csv_source(request, context),
    )
    .map(|document| document.resolved_overlay().clone())
    .map_err(|error| error.to_string())
}

pub(crate) fn overlay_from_source_text(path: &Path, input: &str) -> Result<Overlay, String> {
    let context = source_context_for_path(path)?;
    overlay_from_source_text_with_context(path, input, &context)
}

fn overlay_inventory_document_from_source_text_with_context(
    path: &Path,
    input: &str,
    context: &SourceContext,
) -> Result<OverlaySourceDocument, String> {
    OverlaySourceDocument::parse_with_csv_inventory(
        source_file(path, input, context)?,
        |request| load_source_include(request, context),
        |request| load_csv_source(request, context),
    )
    .map_err(|error| error.to_string())
}

fn overlay_from_source_text_with_context(
    path: &Path,
    input: &str,
    context: &SourceContext,
) -> Result<Overlay, String> {
    overlay_inventory_document_from_source_text_with_context(path, input, context)
        .map(|document| document.resolved_overlay().clone())
}

pub(crate) fn read_manifest(path: &Path) -> Result<manifest::FederatedDeckManifest, String> {
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    manifest::from_str(&input).map_err(|error| format!("{}: {error}", path.display()))
}

pub(crate) fn compose_translation_free_source(
    base: &CanonicalDeck,
    overlays: &[Overlay],
) -> Result<CanonicalDeck, String> {
    let overlays = overlays
        .iter()
        .cloned()
        .map(|mut overlay| {
            overlay.translations = None;
            overlay
        })
        .collect::<Vec<_>>();
    base.compose(&overlays).map_err(|error| error.to_string())
}

pub(crate) fn read_deck_and_overlays(
    deck_path: &Path,
    overlay_paths: &[String],
) -> Result<(CanonicalDeck, Vec<(String, Overlay)>), String> {
    let context = source_context_for_path(deck_path)?;
    let deck = read_deck_with_context(deck_path, &context)?;
    let inventory = overlay_paths
        .iter()
        .map(|path| {
            let input = fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
            Ok((
                path.clone(),
                overlay_inventory_document_from_source_text_with_context(
                    Path::new(path),
                    &input,
                    &context,
                )?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    if inventory
        .iter()
        .all(|(_, document)| document.csv_translation_sources().is_empty())
    {
        return Ok((
            deck,
            inventory
                .into_iter()
                .map(|(path, document)| (path, document.resolved_overlay().clone()))
                .collect(),
        ));
    }
    let source_deck = compose_translation_free_source(
        &deck,
        &inventory
            .iter()
            .map(|(_, document)| document.resolved_overlay().clone())
            .collect::<Vec<_>>(),
    )?;
    let overlays = overlay_paths
        .iter()
        .map(|path| {
            Ok((
                path.clone(),
                read_overlay_with_context(Path::new(path), &context, &source_deck)?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((deck, overlays))
}

pub(crate) fn read_and_compose_deck(
    deck_path: &Path,
    overlay_paths: &[String],
) -> Result<CanonicalDeck, String> {
    let (deck, overlays) = read_deck_and_overlays(deck_path, overlay_paths)?;
    deck.compose(
        &overlays
            .into_iter()
            .map(|(_, overlay)| overlay)
            .collect::<Vec<_>>(),
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn verify_canonical_deck_format(path: &Path) -> Result<(), String> {
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let formatted = canonical_source_document(path, &input)?
        .emit()
        .map_err(|error| error.to_string())?
        .root()
        .text()
        .to_owned();
    if formatted != input {
        return Err(format!("{} is not in canonical format", path.display()));
    }
    Ok(())
}

pub(crate) fn verify_overlay_format(path: &Path) -> Result<(), String> {
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let formatted = overlay_source_document(path, &input)?
        .emit()
        .map_err(|error| error.to_string())?
        .root()
        .text()
        .to_owned();
    if formatted != input {
        return Err(format!("{} is not in canonical format", path.display()));
    }
    Ok(())
}

pub(crate) fn verify_manifest_format(path: &Path) -> Result<(), String> {
    verify_format_with(path, manifest::format_str)
}

fn verify_format_with<E>(
    path: &Path,
    format: impl FnOnce(&str) -> Result<String, E>,
) -> Result<(), String>
where
    E: ToString,
{
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let formatted = source_includes::format_preserving_file_includes(&input, format)?;
    if formatted != input {
        return Err(format!("{} is not in canonical format", path.display()));
    }
    Ok(())
}

pub(crate) fn configured_crowdanki_out(
    manifest: &manifest::FederatedDeckManifest,
    target: &str,
) -> Option<String> {
    manifest
        .targets
        .get(target)?
        .exports
        .crowdanki
        .as_ref()?
        .out
        .clone()
}

pub(crate) fn manifest_root(path: &Path) -> PathBuf {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

pub(crate) struct SourceContext {
    pub(crate) root: PathBuf,
    pub(crate) include_roots: Vec<PathBuf>,
}

pub(crate) fn workspace_root_for_source_path(path: &Path) -> PathBuf {
    nearest_manifest_root(path).unwrap_or_else(|| manifest_root(path))
}

pub(crate) fn source_context_for_path(path: &Path) -> Result<SourceContext, String> {
    let root = workspace_root_for_source_path(path);
    let manifest_path = root.join("brainbrew.yaml");
    let include_roots = if manifest_path.exists() {
        let manifest = read_manifest(&manifest_path)?;
        include_roots_from_manifest(&manifest_path, &root, &manifest)?
    } else {
        Vec::new()
    };
    Ok(SourceContext {
        root,
        include_roots,
    })
}

fn nearest_manifest_root(path: &Path) -> Option<PathBuf> {
    let mut current = path.parent()?;
    loop {
        if current.join("brainbrew.yaml").exists() {
            return Some(if current.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                current.to_path_buf()
            });
        }
        current = current.parent()?;
    }
}

pub(crate) fn include_roots_from_manifest(
    manifest_path: &Path,
    root: &Path,
    manifest: &manifest::FederatedDeckManifest,
) -> Result<Vec<PathBuf>, String> {
    let authorizer = PathAuthorizer::new("package", root)?;
    manifest
        .include_roots
        .iter()
        .enumerate()
        .map(|(index, include_root)| {
            authorizer
                .authorize_read(
                    manifest_path,
                    format!("include_roots[{index}]"),
                    include_root,
                )
                .map(|path| path.into_path_buf())
                .map_err(|error| error.to_string())
        })
        .collect()
}

pub(crate) fn top_level_media_include_path(value: &Value) -> Result<Option<String>, String> {
    let Some(media_value) = value
        .as_mapping()
        .and_then(|mapping| mapping.get(Value::String("media".to_owned())))
    else {
        return Ok(None);
    };
    let Value::Tagged(tagged) = media_value else {
        return Ok(None);
    };
    if tagged.tag != "include" {
        return Ok(None);
    }
    let Value::String(path) = &tagged.value else {
        return Err("media !include path must be a scalar string".to_owned());
    };
    Ok(Some(path.clone()))
}

pub(crate) fn resolve_include_target_for_context(
    include_path: &str,
    context: &SourceContext,
) -> Result<PathBuf, String> {
    authorize_include_read(
        &context.root.join("brainbrew.yaml"),
        "!include",
        include_path,
        context,
    )
}

fn authorize_include_read(
    declaring_file: &Path,
    field: &str,
    include_path: &str,
    context: &SourceContext,
) -> Result<PathBuf, String> {
    let mut roots = vec![("package", context.root.as_path())];
    roots.extend(
        context
            .include_roots
            .iter()
            .map(|root| ("configured include", root.as_path())),
    );
    let mut not_found = None;
    for (name, root) in roots {
        let authorizer = PathAuthorizer::new(name, root)?;
        match authorizer.authorize_read(declaring_file, field, include_path) {
            Ok(path) => return Ok(path.into_path_buf()),
            Err(error) if error.reason.contains("cannot be resolved for reading") => {
                not_found = Some(error.to_string());
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Err(not_found.unwrap_or_else(|| "no selected include root is available".to_owned()))
}
