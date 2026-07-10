use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use brain_brew_core::StableId;
use brain_brew_formats::media;
use brain_brew_formats::source_document::{ImageConversionReport, SourceDocumentEmission};

use crate::commands::lock::validated_nar_hash;
use crate::help;
use crate::io::{
    canonical_document_from_package, format_source_at, manifest_root, overlay_document_from_package,
};
use crate::media_ownership::MediaRootSelections;
use crate::output;
use crate::package_resolver::{DiscoveryPolicy, apply_discovery_option};
use crate::path_authorization::PathAuthorizer;
use crate::planner::{
    ManifestRegistry, MediaDeclarationProvenance, PlanSourceKind, RegistryManifest,
    RegistrySourceKind, SourceProvenance, TargetPlan,
};
use crate::workspace_mutation::{PlannedWorkspaceFile, commit_workspace_files, recover_workspace};

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if matches!(args, [flag] if flag == "--help" || flag == "-h")
        || matches!(args, [_, flag] if flag == "--help" || flag == "-h")
    {
        print!("{}", help::command("media").expect("media help exists"));
        return Ok(());
    }
    match args.first().map(String::as_str) {
        Some("hash") => run_hash(&args[1..]),
        Some("images-to-refs") => run_images_to_refs(&args[1..]),
        _ => Err(help::usage_error(
            "media",
            "usage: brainbrew media hash|images-to-refs --manifest brainbrew.yaml (--all-targets | --target <target>)",
        )),
    }
}

fn run_hash(args: &[String]) -> Result<(), String> {
    let args = parse_media_args(args, true)?;
    let workspace_root = manifest_root(&args.manifest_path);
    recover_workspace(&workspace_root)?;
    let registry = ManifestRegistry::load_with_policy(
        &args.manifest_path,
        &args.include_paths,
        &args.package_roots,
        &args.discovery_policy,
    )?;
    let roots = MediaRootSelections::parse(&registry, &args.media_roots, &workspace_root)?;
    let plans = selected_plans(&registry, args.target.as_deref(), args.all_targets)?;
    let locked_before = snapshot_locked_packages(&registry)?;

    let mut declarations = BTreeMap::<(PathBuf, String), MediaDeclarationProvenance>::new();
    for plan in &plans {
        roots.require_for_plan(plan)?;
        for declaration in plan.media_declarations.values() {
            if !declaration.source_kind.is_root_workspace() {
                return Err(read_only_declaration_error("media hash", plan, declaration));
            }
            let key = (declaration.source.clone(), declaration.id.clone());
            if let Some(previous) = declarations.insert(key, declaration.clone())
                && (previous.path != declaration.path || previous.sha256 != declaration.sha256)
            {
                return Err(format!(
                    "media hash found inconsistent final declarations for {} in {}",
                    declaration.id,
                    declaration.source.display()
                ));
            }
        }
    }

    let mut updates = BTreeMap::<PathBuf, Vec<(MediaDeclarationProvenance, String)>>::new();
    for declaration in declarations.into_values() {
        let root = roots.require_for_declaration("media-hash mutation", &declaration)?;
        let authorizer = PathAuthorizer::new(
            format!("media for package {}", declaration.package_label()),
            root,
        )?;
        let asset = authorizer
            .authorize_read(
                &declaration.source,
                format!("media.{}.path", declaration.id),
                &declaration.path,
            )
            .map_err(|error| mutation_asset_error(&declaration, root, &error.to_string()))?
            .into_path_buf();
        let bytes = fs::read(&asset)
            .map_err(|error| mutation_asset_error(&declaration, root, &error.to_string()))?;
        let actual = media::sha256_hex(&bytes);
        if actual != declaration.sha256 {
            updates
                .entry(declaration.document_source.clone())
                .or_default()
                .push((declaration, actual));
        }
    }

    let changed_entries = updates.values().map(Vec::len).sum::<usize>();
    let mut replacements = BTreeMap::<PathBuf, Vec<u8>>::new();
    for (document_path, changes) in updates {
        let source = plan_source(&plans, &document_path)?;
        let loaded = registry_manifest_for_source(&registry, source)?;
        match source.kind {
            PlanSourceKind::Base => {
                let mut document = canonical_document_from_package(
                    &document_path,
                    &loaded.root,
                    &loaded.include_roots,
                )?;
                for (declaration, hash) in changes {
                    document
                        .set_media_hash(
                            &StableId::new(&declaration.id).map_err(|error| error.to_string())?,
                            &declaration.path,
                            &hash,
                        )
                        .map_err(|error| error.to_string())?;
                }
                collect_emission(
                    document.emit().map_err(|error| error.to_string())?,
                    &mut replacements,
                )?;
            }
            PlanSourceKind::Overlay { .. } => {
                let mut document = overlay_document_from_package(
                    &document_path,
                    &loaded.root,
                    &loaded.include_roots,
                )?;
                for (declaration, hash) in changes {
                    document
                        .set_media_hash(
                            &StableId::new(&declaration.id).map_err(|error| error.to_string())?,
                            &declaration.path,
                            &hash,
                        )
                        .map_err(|error| error.to_string())?;
                }
                collect_emission(
                    document.emit().map_err(|error| error.to_string())?,
                    &mut replacements,
                )?;
            }
            _ => {
                return Err(format!(
                    "{} is not a mutable root source document",
                    document_path.display()
                ));
            }
        }
    }

    ensure_locked_packages_unchanged(&registry, &locked_before)?;
    let files = planned_replacements(replacements)?;
    let changed_files = files.len();
    commit_workspace_files(&workspace_root, files)?;
    ensure_locked_packages_unchanged(&registry, &locked_before)?;
    output::print_success(
        "updated media hashes",
        &[
            ("manifest", args.manifest_path.display().to_string()),
            ("media roots", args.media_roots.join(", ")),
            ("targets", plans.len().to_string()),
            ("files changed", changed_files.to_string()),
            ("entries changed", changed_entries.to_string()),
        ],
    );
    Ok(())
}

fn run_images_to_refs(args: &[String]) -> Result<(), String> {
    let args = parse_media_args(args, false)?;
    let workspace_root = manifest_root(&args.manifest_path);
    recover_workspace(&workspace_root)?;
    let registry = ManifestRegistry::load_with_policy(
        &args.manifest_path,
        &args.include_paths,
        &args.package_roots,
        &args.discovery_policy,
    )?;
    let plans = selected_plans(&registry, args.target.as_deref(), args.all_targets)?;
    let locked_before = snapshot_locked_packages(&registry)?;
    let lookup = media_path_lookup(&plans);
    let sources = mutable_candidate_sources(&plans);
    let mut replacements = BTreeMap::<PathBuf, Vec<u8>>::new();
    let mut report = ImageConversionReport::default();

    for source in sources.values() {
        let loaded = registry_manifest_for_source(&registry, source)?;
        match source.kind {
            PlanSourceKind::Base => {
                let mut document = canonical_document_from_package(
                    &source.path,
                    &loaded.root,
                    &loaded.include_roots,
                )?;
                let local = document
                    .convert_strict_image_fields(&lookup)
                    .map_err(|error| error.to_string())?;
                guard_source_mutability("media images-to-refs", source, local.converted)?;
                add_report(&mut report, &local);
                if local.converted > 0 {
                    collect_emission(
                        document.emit().map_err(|error| error.to_string())?,
                        &mut replacements,
                    )?;
                }
            }
            PlanSourceKind::Overlay { .. } => {
                let mut document = overlay_document_from_package(
                    &source.path,
                    &loaded.root,
                    &loaded.include_roots,
                )?;
                let local = document
                    .convert_strict_image_fields(&lookup)
                    .map_err(|error| error.to_string())?;
                guard_source_mutability("media images-to-refs", source, local.converted)?;
                add_report(&mut report, &local);
                if local.converted > 0 {
                    collect_emission(
                        document.emit().map_err(|error| error.to_string())?,
                        &mut replacements,
                    )?;
                }
            }
            _ => {}
        }
    }

    ensure_locked_packages_unchanged(&registry, &locked_before)?;
    let files = planned_replacements(replacements)?;
    let changed_files = files.len();
    commit_workspace_files(&workspace_root, files)?;
    ensure_locked_packages_unchanged(&registry, &locked_before)?;
    output::print_success(
        "converted strict image fields to !image references",
        &[
            ("manifest", args.manifest_path.display().to_string()),
            ("targets", plans.len().to_string()),
            ("files changed", changed_files.to_string()),
            ("converted fields", report.converted.to_string()),
            (
                "skipped non-strict image fields",
                report.skipped_non_strict.to_string(),
            ),
            (
                "skipped no media match",
                report.skipped_no_match.to_string(),
            ),
            (
                "skipped ambiguous media path",
                report.skipped_ambiguous_path.to_string(),
            ),
        ],
    );
    Ok(())
}

struct MediaArgs {
    manifest_path: PathBuf,
    target: Option<String>,
    all_targets: bool,
    media_roots: Vec<String>,
    include_paths: Vec<PathBuf>,
    package_roots: Vec<PathBuf>,
    discovery_policy: DiscoveryPolicy,
}

fn parse_media_args(args: &[String], require_roots: bool) -> Result<MediaArgs, String> {
    let mut parsed = MediaArgs {
        manifest_path: PathBuf::from("brainbrew.yaml"),
        target: None,
        all_targets: false,
        media_roots: Vec::new(),
        include_paths: Vec::new(),
        package_roots: Vec::new(),
        discovery_policy: DiscoveryPolicy::default(),
    };
    let mut index = 0;
    while index < args.len() {
        let value = |name: &str, index: usize| {
            args.get(index + 1)
                .cloned()
                .ok_or_else(|| format!("{name} requires a value"))
        };
        match args[index].as_str() {
            "--manifest" => {
                parsed.manifest_path = PathBuf::from(value("--manifest", index)?);
                index += 2;
            }
            "--target" => {
                parsed.target = Some(value("--target", index)?);
                index += 2;
            }
            "--all-targets" => {
                parsed.all_targets = true;
                index += 1;
            }
            "--media-root" => {
                parsed.media_roots.push(value("--media-root", index)?);
                index += 2;
            }
            "--include" => {
                parsed
                    .include_paths
                    .push(PathBuf::from(value("--include", index)?));
                index += 2;
            }
            "--package-root" => {
                parsed
                    .package_roots
                    .push(PathBuf::from(value("--package-root", index)?));
                index += 2;
            }
            flag @ ("--discovery-max-depth"
            | "--discovery-max-entries"
            | "--discovery-max-manifests"
            | "--package-ignore") => {
                let selected = value(flag, index)?;
                apply_discovery_option(flag, &selected, &mut parsed.discovery_policy)?;
                index += 2;
            }
            other => return Err(format!("unexpected media argument {other:?}")),
        }
    }
    if parsed.all_targets == parsed.target.is_some() {
        return Err(
            "media command requires exactly one of --all-targets or --target <target>".to_owned(),
        );
    }
    if require_roots && parsed.media_roots.is_empty() {
        return Err("media hash requires --media-root".to_owned());
    }
    if !require_roots && !parsed.media_roots.is_empty() {
        return Err("media images-to-refs does not use --media-root".to_owned());
    }
    Ok(parsed)
}

fn selected_plans(
    registry: &ManifestRegistry,
    target: Option<&str>,
    all_targets: bool,
) -> Result<Vec<TargetPlan>, String> {
    let references = if all_targets {
        registry
            .root()
            .manifest
            .targets
            .keys()
            .map(|target| {
                registry
                    .root()
                    .identity
                    .as_ref()
                    .map(|identity| format!("{}:{target}", identity.id))
                    .unwrap_or_else(|| target.clone())
            })
            .collect::<Vec<_>>()
    } else {
        vec![target.expect("parser requires target").to_owned()]
    };
    references
        .iter()
        .map(|target| registry.plan(target))
        .collect()
}

fn mutable_candidate_sources(plans: &[TargetPlan]) -> BTreeMap<PathBuf, SourceProvenance> {
    let mut sources = BTreeMap::new();
    for plan in plans {
        for source in plan.sources() {
            if matches!(
                source.kind,
                PlanSourceKind::Base | PlanSourceKind::Overlay { .. }
            ) {
                sources
                    .entry(source.path.clone())
                    .or_insert_with(|| source.clone());
            }
        }
    }
    sources
}

fn media_path_lookup(plans: &[TargetPlan]) -> BTreeMap<String, Option<StableId>> {
    let mut lookup = BTreeMap::new();
    for declaration in plans
        .iter()
        .flat_map(|plan| plan.media_declarations.values())
    {
        let id = StableId::new(&declaration.id).expect("planned media ID is valid");
        lookup
            .entry(declaration.path.clone())
            .and_modify(|existing: &mut Option<StableId>| {
                if existing.as_ref() != Some(&id) {
                    *existing = None;
                }
            })
            .or_insert(Some(id));
    }
    lookup
}

fn registry_manifest_for_source<'a>(
    registry: &'a ManifestRegistry,
    source: &SourceProvenance,
) -> Result<&'a RegistryManifest, String> {
    registry
        .manifests()
        .iter()
        .find(|loaded| loaded.path == source.manifest && loaded.root == source.package_root)
        .ok_or_else(|| format!("no registry package owns source {}", source.path.display()))
}

fn plan_source<'a>(plans: &'a [TargetPlan], path: &Path) -> Result<&'a SourceProvenance, String> {
    plans
        .iter()
        .flat_map(TargetPlan::sources)
        .find(|source| source.path == path)
        .ok_or_else(|| format!("no target plan owns source {}", path.display()))
}

fn collect_emission(
    emission: SourceDocumentEmission,
    replacements: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), String> {
    collect_source_file(emission.root(), replacements)?;
    for included in emission.included() {
        collect_source_file(included, replacements)?;
    }
    Ok(())
}

fn collect_source_file(
    source: &brain_brew_formats::source_document::SourceFile,
    replacements: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), String> {
    let path = PathBuf::from(source.provenance().source_name());
    let bytes = source.text().as_bytes().to_vec();
    if let Some(previous) = replacements.insert(path.clone(), bytes.clone())
        && previous != bytes
    {
        return Err(format!(
            "conflicting mutation outputs for {}",
            path.display()
        ));
    }
    Ok(())
}

fn planned_replacements(
    replacements: BTreeMap<PathBuf, Vec<u8>>,
) -> Result<Vec<PlannedWorkspaceFile>, String> {
    let mut planned = Vec::new();
    for (path, replacement) in replacements {
        let original = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        if original == replacement {
            continue;
        }
        let validation_path = path.clone();
        planned.push(PlannedWorkspaceFile::validated(
            path,
            original,
            replacement,
            move |bytes| {
                let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
                let formatted = format_source_at(&validation_path, text)?;
                if formatted == text {
                    Ok(())
                } else {
                    Err("replacement is not canonical Brain Brew source".to_owned())
                }
            },
        )?);
    }
    Ok(planned)
}

fn guard_source_mutability(
    command: &str,
    source: &SourceProvenance,
    changes: usize,
) -> Result<(), String> {
    if changes == 0 || source.registry_source.is_root_workspace() {
        return Ok(());
    }
    Err(format!(
        "{command} is read-only for {} package source {} ({}) at {}; requested operation would mutate {changes} field(s). Dependency/include/locked sources cannot be vendored by this command",
        source
            .package
            .as_ref()
            .map(|package| package.id.as_str())
            .unwrap_or("unnamed"),
        source.registry_source.ownership_name(),
        source.kind_name(),
        source.path.display()
    ))
}

trait SourceKindName {
    fn kind_name(&self) -> &'static str;
}

impl SourceKindName for SourceProvenance {
    fn kind_name(&self) -> &'static str {
        match self.kind {
            PlanSourceKind::Base => "base",
            PlanSourceKind::Overlay { .. } => "overlay",
            PlanSourceKind::ScalarInclude { .. } => "scalar include",
            PlanSourceKind::MediaInclude => "media include",
        }
    }
}

fn read_only_declaration_error(
    command: &str,
    plan: &TargetPlan,
    declaration: &MediaDeclarationProvenance,
) -> String {
    format!(
        "{command} is read-only for target {}, package {}, declaration {}, path {:?}, source {} ({}) at {}; dependency/include/locked declarations cannot be mutated",
        plan.qualified_name,
        declaration.package_label(),
        declaration.id,
        declaration.path,
        declaration.source_kind.ownership_name(),
        declaration.source_kind.ownership_name(),
        declaration.source.display()
    )
}

fn mutation_asset_error(
    declaration: &MediaDeclarationProvenance,
    root: &Path,
    reason: &str,
) -> String {
    format!(
        "media hash asset error for package {}, declaration {}, path {:?}, root {}: {reason}",
        declaration.package_label(),
        declaration.id,
        declaration.path,
        root.display()
    )
}

fn snapshot_locked_packages(
    registry: &ManifestRegistry,
) -> Result<BTreeMap<PathBuf, String>, String> {
    registry
        .manifests()
        .iter()
        .filter(|loaded| loaded.discovery == RegistrySourceKind::SiblingLock)
        .map(|loaded| {
            Ok((
                loaded.root.clone(),
                validated_nar_hash(&loaded.root, "locked package tree")?,
            ))
        })
        .collect()
}

fn ensure_locked_packages_unchanged(
    registry: &ManifestRegistry,
    expected: &BTreeMap<PathBuf, String>,
) -> Result<(), String> {
    let actual = snapshot_locked_packages(registry)?;
    if &actual == expected {
        Ok(())
    } else {
        Err(format!(
            "locked/cache package integrity changed during media mutation; expected {expected:?}, found {actual:?}; cache was not repaired"
        ))
    }
}

fn add_report(total: &mut ImageConversionReport, local: &ImageConversionReport) {
    total.converted += local.converted;
    total.skipped_non_strict += local.skipped_non_strict;
    total.skipped_no_match += local.skipped_no_match;
    total.skipped_ambiguous_path += local.skipped_ambiguous_path;
}
