use std::path::PathBuf;

use brain_brew_core::{OverlayKind, StableId};
use brain_brew_formats::manifest::TranslationCoveragePolicy;

pub(crate) struct ManifestTargetArgs {
    pub(crate) manifest_path: PathBuf,
    pub(crate) target: String,
    pub(crate) out_path: Option<PathBuf>,
    pub(crate) media_roots: Vec<String>,
    pub(crate) include_paths: Vec<PathBuf>,
    pub(crate) package_roots: Vec<PathBuf>,
}

pub(crate) struct VerifyArgs {
    pub(crate) manifest_path: PathBuf,
    pub(crate) target: Option<String>,
    pub(crate) all_targets: bool,
    pub(crate) media_roots: Vec<String>,
    pub(crate) include_paths: Vec<PathBuf>,
    pub(crate) package_roots: Vec<PathBuf>,
    pub(crate) translation_coverage: Option<TranslationCoveragePolicy>,
    pub(crate) skip_content_validation: bool,
}

pub(crate) struct ExportArgs {
    pub(crate) overlay_paths: Vec<String>,
    pub(crate) out_path: Option<PathBuf>,
    pub(crate) media_root: Option<PathBuf>,
}

pub(crate) struct DiffOverlayArgs {
    pub(crate) left_path: PathBuf,
    pub(crate) right_path: PathBuf,
    pub(crate) id: StableId,
    pub(crate) kind: OverlayKind,
}

pub(crate) struct TargetsArgs {
    pub(crate) manifest_paths: Vec<PathBuf>,
    pub(crate) package_roots: Vec<PathBuf>,
}

pub(crate) fn split_json_flag(args: &[String]) -> (bool, Vec<String>) {
    let mut json_output = false;
    let mut rest = Vec::new();
    for arg in args {
        if arg == "--json" {
            json_output = true;
        } else {
            rest.push(arg.clone());
        }
    }
    (json_output, rest)
}

pub(crate) fn parse_targets_args(args: &[String]) -> Result<TargetsArgs, String> {
    let mut manifest_paths = Vec::new();
    let mut package_roots = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" | "--include" => {
                let Some(path) = args.get(index + 1) else {
                    return Err(format!("{} requires a path", args[index]));
                };
                manifest_paths.push(PathBuf::from(path));
                index += 2;
            }
            "--package-root" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--package-root requires a path".to_owned());
                };
                package_roots.push(PathBuf::from(path));
                index += 2;
            }
            other => return Err(format!("unexpected targets argument {other:?}")),
        }
    }
    if manifest_paths.is_empty() && package_roots.is_empty() {
        manifest_paths.push(PathBuf::from("brainbrew.yaml"));
    }
    Ok(TargetsArgs {
        manifest_paths,
        package_roots,
    })
}

pub(crate) fn parse_manifest_target_args(args: &[String]) -> Result<ManifestTargetArgs, String> {
    let mut manifest_path = None;
    let mut target = None;
    let mut out_path = None;
    let mut media_roots = Vec::new();
    let mut include_paths = Vec::new();
    let mut package_roots = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--manifest requires a path".to_owned());
                };
                manifest_path = Some(PathBuf::from(path));
                index += 2;
            }
            "--target" => {
                let Some(name) = args.get(index + 1) else {
                    return Err("--target requires a name".to_owned());
                };
                target = Some(name.clone());
                index += 2;
            }
            "--out" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--out requires a path".to_owned());
                };
                out_path = Some(PathBuf::from(path));
                index += 2;
            }
            "--media-root" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--media-root requires a path".to_owned());
                };
                media_roots.push(path.clone());
                index += 2;
            }
            "--include" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--include requires a path".to_owned());
                };
                include_paths.push(PathBuf::from(path));
                index += 2;
            }
            "--package-root" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--package-root requires a path".to_owned());
                };
                package_roots.push(PathBuf::from(path));
                index += 2;
            }
            other => return Err(format!("unexpected argument {other:?}")),
        }
    }
    let Some(target) = target else {
        return Err("missing --target".to_owned());
    };
    Ok(ManifestTargetArgs {
        manifest_path: manifest_path.unwrap_or_else(|| PathBuf::from("brainbrew.yaml")),
        target,
        out_path,
        media_roots,
        include_paths,
        package_roots,
    })
}

pub(crate) fn parse_verify_args(args: &[String]) -> Result<VerifyArgs, String> {
    let mut manifest_path = None;
    let mut target = None;
    let mut all_targets = false;
    let mut media_roots = Vec::new();
    let mut include_paths = Vec::new();
    let mut package_roots = Vec::new();
    let mut translation_coverage = None;
    let mut skip_content_validation = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--manifest requires a path".to_owned());
                };
                manifest_path = Some(PathBuf::from(path));
                index += 2;
            }
            "--target" => {
                let Some(name) = args.get(index + 1) else {
                    return Err("--target requires a name".to_owned());
                };
                target = Some(name.clone());
                index += 2;
            }
            "--all-targets" => {
                all_targets = true;
                index += 1;
            }
            "--media-root" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--media-root requires a path".to_owned());
                };
                media_roots.push(path.clone());
                index += 2;
            }
            "--include" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--include requires a path".to_owned());
                };
                include_paths.push(PathBuf::from(path));
                index += 2;
            }
            "--package-root" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--package-root requires a path".to_owned());
                };
                package_roots.push(PathBuf::from(path));
                index += 2;
            }
            "--translation-coverage" => {
                let Some(policy) = args.get(index + 1) else {
                    return Err("--translation-coverage requires lenient or strict".to_owned());
                };
                translation_coverage = Some(parse_translation_coverage_policy(policy)?);
                index += 2;
            }
            "--skip-content-validation" => {
                skip_content_validation = true;
                index += 1;
            }
            other => return Err(format!("unexpected verify argument {other:?}")),
        }
    }
    if all_targets && target.is_some() {
        return Err("choose --all-targets or --target, not both".to_owned());
    }
    Ok(VerifyArgs {
        manifest_path: manifest_path.unwrap_or_else(|| PathBuf::from("brainbrew.yaml")),
        target,
        all_targets,
        media_roots,
        include_paths,
        package_roots,
        translation_coverage,
        skip_content_validation,
    })
}

fn parse_translation_coverage_policy(value: &str) -> Result<TranslationCoveragePolicy, String> {
    match value {
        "lenient" => Ok(TranslationCoveragePolicy::Lenient),
        "strict" => Ok(TranslationCoveragePolicy::Strict),
        other => Err(format!(
            "invalid translation coverage policy {other:?}; expected lenient or strict"
        )),
    }
}

pub(crate) fn parse_overlay_and_optional_out(
    args: &[String],
) -> Result<(Vec<String>, Option<PathBuf>), String> {
    let export_args = parse_overlay_out_media(args)?;
    if export_args.media_root.is_some() {
        return Err("--media-root is only supported for media-aware commands".to_owned());
    }
    Ok((export_args.overlay_paths, export_args.out_path))
}

pub(crate) fn parse_overlay_out_media(args: &[String]) -> Result<ExportArgs, String> {
    let mut overlay_paths = Vec::new();
    let mut out_path = None;
    let mut media_root = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--overlay" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--overlay requires a path".to_owned());
                };
                overlay_paths.push(path.clone());
                index += 2;
            }
            "--out" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--out requires a path".to_owned());
                };
                out_path = Some(PathBuf::from(path));
                index += 2;
            }
            "--media-root" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--media-root requires a path".to_owned());
                };
                media_root = Some(PathBuf::from(path));
                index += 2;
            }
            other => return Err(format!("unexpected argument {other:?}")),
        }
    }
    Ok(ExportArgs {
        overlay_paths,
        out_path,
        media_root,
    })
}

pub(crate) fn parse_required_out(args: &[String]) -> Result<PathBuf, String> {
    let Some(index) = args.iter().position(|arg| arg == "--out") else {
        return Err("missing --out".to_owned());
    };
    let Some(path) = args.get(index + 1) else {
        return Err("--out requires a path".to_owned());
    };
    Ok(PathBuf::from(path))
}

pub(crate) fn parse_diff_overlay_args(args: &[String]) -> Result<DiffOverlayArgs, String> {
    let mut paths = Vec::new();
    let mut id = None;
    let mut kind = OverlayKind::Patch;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--as-overlay" => index += 1,
            "--id" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--id requires an overlay stable id".to_owned());
                };
                id = Some(stable_id(value)?);
                index += 2;
            }
            "--kind" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--kind requires an overlay kind".to_owned());
                };
                kind = parse_overlay_kind(value)?;
                index += 2;
            }
            other if !other.starts_with('-') => {
                paths.push(PathBuf::from(other));
                index += 1;
            }
            other => return Err(format!("unexpected argument {other:?}")),
        }
    }
    if paths.len() != 2 {
        return Err(
            "usage: brainbrew diff <left.yaml> <right.yaml> --as-overlay --id <overlay-id> [--kind patch]"
                .to_owned(),
        );
    }
    let Some(id) = id else {
        return Err("diff --as-overlay requires --id".to_owned());
    };
    Ok(DiffOverlayArgs {
        left_path: paths.remove(0),
        right_path: paths.remove(0),
        id,
        kind,
    })
}

fn parse_overlay_kind(value: &str) -> Result<OverlayKind, String> {
    match value {
        "translation" => Ok(OverlayKind::Translation),
        "extension" => Ok(OverlayKind::Extension),
        "patch" => Ok(OverlayKind::Patch),
        "personal" => Ok(OverlayKind::Personal),
        other => Err(format!("unknown overlay kind {other:?}")),
    }
}

fn stable_id(value: &str) -> Result<StableId, String> {
    StableId::new(value).map_err(|error| error.to_string())
}
