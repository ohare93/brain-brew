use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use brain_brew_formats::media;
use serde_yaml::{Mapping, Value};

use crate::help;
use crate::io::{
    format_source_at, manifest_root, plan_manifest_target_with_packages, read_manifest,
    root_relative_path,
};
use crate::output;

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if matches!(args, [flag] if flag == "--help" || flag == "-h")
        || matches!(args, [subcommand, flag] if subcommand == "hash" && (flag == "--help" || flag == "-h"))
    {
        print!("{}", help::command("media").expect("media help exists"));
        return Ok(());
    }
    if args.first().map(String::as_str) != Some("hash") {
        return Err(help::usage_error(
            "media",
            "usage: brainbrew media hash --manifest brainbrew.yaml (--all-targets | --target <target>) --media-root media/",
        ));
    }

    let hash_args = parse_media_hash_args(&args[1..])?;
    let manifest = read_manifest(&hash_args.manifest_path)?;
    let root = manifest_root(&hash_args.manifest_path);
    let media_root = root_relative_path(&root, &hash_args.media_root);
    let target_names = if hash_args.all_targets {
        manifest.targets.keys().cloned().collect::<Vec<_>>()
    } else if let Some(target) = hash_args.target {
        vec![target]
    } else {
        return Err("media hash requires --all-targets or --target <target>".to_owned());
    };

    let mut source_files = BTreeSet::from([root.join(&manifest.base)]);
    for target in &target_names {
        let plan = plan_manifest_target_with_packages(
            &hash_args.manifest_path,
            target,
            &hash_args.include_paths,
            &hash_args.package_roots,
        )?;
        for (overlay, _) in plan.overlays {
            source_files.insert(overlay.file);
        }
    }

    let mut changed_entries = 0usize;
    let mut changed_files = 0usize;
    for path in source_files {
        let input =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let mut value = serde_yaml::from_str::<Value>(&input)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let updates = update_media_hashes_in_value(&mut value, &media_root)?;
        if updates == 0 {
            continue;
        }
        let updated_yaml = serde_yaml::to_string(&value)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let formatted = format_source_at(&path, &updated_yaml)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        fs::write(&path, formatted).map_err(|error| format!("{}: {error}", path.display()))?;
        changed_entries += updates;
        changed_files += 1;
    }

    output::print_success(
        "updated media hashes",
        &[
            ("manifest", hash_args.manifest_path.display().to_string()),
            ("media root", media_root.display().to_string()),
            ("files changed", changed_files.to_string()),
            ("entries changed", changed_entries.to_string()),
        ],
    );
    Ok(())
}

struct MediaHashArgs {
    manifest_path: PathBuf,
    target: Option<String>,
    all_targets: bool,
    media_root: PathBuf,
    include_paths: Vec<PathBuf>,
    package_roots: Vec<PathBuf>,
}

fn parse_media_hash_args(args: &[String]) -> Result<MediaHashArgs, String> {
    let mut manifest_path = None;
    let mut target = None;
    let mut all_targets = false;
    let mut media_root = None;
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
            "--all-targets" => {
                all_targets = true;
                index += 1;
            }
            "--media-root" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--media-root requires a path".to_owned());
                };
                media_root = Some(PathBuf::from(path));
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
            other => return Err(format!("unexpected media hash argument {other:?}")),
        }
    }
    if all_targets && target.is_some() {
        return Err("choose --all-targets or --target, not both".to_owned());
    }
    let Some(media_root) = media_root else {
        return Err("media hash requires --media-root".to_owned());
    };
    Ok(MediaHashArgs {
        manifest_path: manifest_path.unwrap_or_else(|| PathBuf::from("brainbrew.yaml")),
        target,
        all_targets,
        media_root,
        include_paths,
        package_roots,
    })
}

fn update_media_hashes_in_value(value: &mut Value, media_root: &Path) -> Result<usize, String> {
    let Some(media_entries) = value
        .as_mapping_mut()
        .and_then(|mapping| mapping.get_mut(Value::String("media".to_owned())))
        .and_then(Value::as_mapping_mut)
    else {
        return Ok(0);
    };

    let mut updates = 0;
    for (_id, entry) in media_entries {
        let Some(entry) = entry.as_mapping_mut() else {
            continue;
        };
        let Some(path) = string_field(entry, "path") else {
            continue;
        };
        let relative_path = safe_media_relative_path(&path)?;
        let full_path = media_root.join(relative_path);
        let bytes =
            fs::read(&full_path).map_err(|error| format!("{}: {error}", full_path.display()))?;
        let actual = media::sha256_hex(&bytes);
        let key = Value::String("sha256".to_owned());
        if entry.get(&key).and_then(Value::as_str) != Some(actual.as_str()) {
            entry.insert(key, Value::String(actual));
            updates += 1;
        }
    }
    Ok(updates)
}

fn string_field(mapping: &Mapping, key: &str) -> Option<String> {
    mapping
        .get(Value::String(key.to_owned()))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn safe_media_relative_path(path: &str) -> Result<PathBuf, String> {
    let path = Path::new(path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(format!("unsafe media path {}", path.display()));
    }
    Ok(path.to_path_buf())
}
