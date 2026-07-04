use std::collections::{BTreeMap, BTreeSet};
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
    let hash_args = parse_media_hash_args(args)?;
    let root = manifest_root(&hash_args.manifest_path);
    let media_root = root_relative_path(&root, &hash_args.media_root);
    let (target_names, source_files) = collect_manifest_source_files(
        &hash_args.manifest_path,
        hash_args.target.as_deref(),
        hash_args.all_targets,
        &hash_args.include_paths,
        &hash_args.package_roots,
    )?;

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
            ("targets", target_names.len().to_string()),
            ("files changed", changed_files.to_string()),
            ("entries changed", changed_entries.to_string()),
        ],
    );
    Ok(())
}

fn run_images_to_refs(args: &[String]) -> Result<(), String> {
    let migration_args = parse_media_migration_args(args)?;
    let (target_names, source_files) = collect_manifest_source_files(
        &migration_args.manifest_path,
        migration_args.target.as_deref(),
        migration_args.all_targets,
        &migration_args.include_paths,
        &migration_args.package_roots,
    )?;
    let media_path_lookup = media_path_lookup_from_sources(&source_files)?;

    let mut report = ImageMigrationReport::default();
    for path in source_files {
        let input =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let mut value = serde_yaml::from_str::<Value>(&input)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let before = report.converted;
        migrate_image_fields_in_source(&mut value, &media_path_lookup, &mut report);
        if report.converted == before {
            continue;
        }
        let updated_yaml = serde_yaml::to_string(&value)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let formatted = format_source_at(&path, &updated_yaml)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        fs::write(&path, formatted).map_err(|error| format!("{}: {error}", path.display()))?;
        report.files_changed += 1;
    }

    output::print_success(
        "converted strict image fields to !image references",
        &[
            (
                "manifest",
                migration_args.manifest_path.display().to_string(),
            ),
            ("targets", target_names.len().to_string()),
            ("files changed", report.files_changed.to_string()),
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

struct MediaHashArgs {
    manifest_path: PathBuf,
    target: Option<String>,
    all_targets: bool,
    media_root: PathBuf,
    include_paths: Vec<PathBuf>,
    package_roots: Vec<PathBuf>,
}

struct MediaMigrationArgs {
    manifest_path: PathBuf,
    target: Option<String>,
    all_targets: bool,
    include_paths: Vec<PathBuf>,
    package_roots: Vec<PathBuf>,
}

fn parse_media_hash_args(args: &[String]) -> Result<MediaHashArgs, String> {
    let mut parsed = parse_media_manifest_args(args, "media hash")?;
    let Some(media_root) = parsed.media_root.take() else {
        return Err("media hash requires --media-root".to_owned());
    };
    Ok(MediaHashArgs {
        manifest_path: parsed.manifest_path,
        target: parsed.target,
        all_targets: parsed.all_targets,
        media_root,
        include_paths: parsed.include_paths,
        package_roots: parsed.package_roots,
    })
}

fn parse_media_migration_args(args: &[String]) -> Result<MediaMigrationArgs, String> {
    let parsed = parse_media_manifest_args(args, "media images-to-refs")?;
    if parsed.media_root.is_some() {
        return Err("media images-to-refs does not use --media-root".to_owned());
    }
    Ok(MediaMigrationArgs {
        manifest_path: parsed.manifest_path,
        target: parsed.target,
        all_targets: parsed.all_targets,
        include_paths: parsed.include_paths,
        package_roots: parsed.package_roots,
    })
}

struct ParsedMediaManifestArgs {
    manifest_path: PathBuf,
    target: Option<String>,
    all_targets: bool,
    media_root: Option<PathBuf>,
    include_paths: Vec<PathBuf>,
    package_roots: Vec<PathBuf>,
}

fn parse_media_manifest_args(
    args: &[String],
    command_name: &str,
) -> Result<ParsedMediaManifestArgs, String> {
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
            other => return Err(format!("unexpected {command_name} argument {other:?}")),
        }
    }
    if all_targets && target.is_some() {
        return Err("choose --all-targets or --target, not both".to_owned());
    }
    Ok(ParsedMediaManifestArgs {
        manifest_path: manifest_path.unwrap_or_else(|| PathBuf::from("brainbrew.yaml")),
        target,
        all_targets,
        media_root,
        include_paths,
        package_roots,
    })
}

fn collect_manifest_source_files(
    manifest_path: &Path,
    target: Option<&str>,
    all_targets: bool,
    include_paths: &[PathBuf],
    package_roots: &[PathBuf],
) -> Result<(Vec<String>, BTreeSet<PathBuf>), String> {
    let manifest = read_manifest(manifest_path)?;
    let root = manifest_root(manifest_path);
    let target_names = if all_targets {
        manifest.targets.keys().cloned().collect::<Vec<_>>()
    } else if let Some(target) = target {
        vec![target.to_owned()]
    } else {
        return Err("media command requires --all-targets or --target <target>".to_owned());
    };

    let mut source_files = BTreeSet::from([root.join(&manifest.base)]);
    for target in &target_names {
        let plan = plan_manifest_target_with_packages(
            manifest_path,
            target,
            include_paths,
            package_roots,
        )?;
        for (overlay, _) in plan.overlays {
            source_files.insert(overlay.file);
        }
    }

    Ok((target_names, source_files))
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

#[derive(Default)]
struct ImageMigrationReport {
    converted: usize,
    files_changed: usize,
    skipped_non_strict: usize,
    skipped_no_match: usize,
    skipped_ambiguous_path: usize,
}

fn media_path_lookup_from_sources(
    source_files: &BTreeSet<PathBuf>,
) -> Result<BTreeMap<String, Option<String>>, String> {
    let mut lookup = BTreeMap::new();
    for path in source_files {
        let input =
            fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let value = serde_yaml::from_str::<Value>(&input)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        collect_media_paths_from_value(&value, &mut lookup);
    }
    Ok(lookup)
}

fn collect_media_paths_from_value(value: &Value, lookup: &mut BTreeMap<String, Option<String>>) {
    let Some(media_entries) = value
        .as_mapping()
        .and_then(|mapping| mapping.get(Value::String("media".to_owned())))
        .and_then(Value::as_mapping)
    else {
        return;
    };

    for (id, entry) in media_entries {
        let (Some(id), Some(entry)) = (id.as_str(), entry.as_mapping()) else {
            continue;
        };
        let Some(path) = string_field(entry, "path") else {
            continue;
        };
        lookup
            .entry(path)
            .and_modify(|existing| {
                if existing.as_deref() != Some(id) {
                    *existing = None;
                }
            })
            .or_insert_with(|| Some(id.to_owned()));
    }
}

fn migrate_image_fields_in_source(
    value: &mut Value,
    media_path_lookup: &BTreeMap<String, Option<String>>,
    report: &mut ImageMigrationReport,
) {
    let Some(root) = value.as_mapping_mut() else {
        return;
    };

    if let Some(notes) = mapping_value_mut(root, "notes").and_then(Value::as_mapping_mut) {
        for (_note_id, note_value) in notes {
            migrate_note_like_value(note_value, media_path_lookup, report);
        }
    }

    if let Some(field_additions) =
        mapping_value_mut(root, "field_additions").and_then(Value::as_mapping_mut)
    {
        for (_note_type_id, additions_value) in field_additions {
            let Some(additions) = additions_value.as_mapping_mut() else {
                continue;
            };
            let Some(values) =
                mapping_value_mut(additions, "values").and_then(Value::as_mapping_mut)
            else {
                continue;
            };
            for (_note_id, fields_value) in values {
                migrate_field_mapping(fields_value, media_path_lookup, report);
            }
        }
    }

    if let Some(field_fills) =
        mapping_value_mut(root, "field_fills").and_then(Value::as_mapping_mut)
    {
        for (_note_id, fields_value) in field_fills {
            migrate_field_mapping(fields_value, media_path_lookup, report);
        }
    }
}

fn migrate_note_like_value(
    note_value: &mut Value,
    media_path_lookup: &BTreeMap<String, Option<String>>,
    report: &mut ImageMigrationReport,
) {
    let Some(note) = note_value.as_mapping_mut() else {
        return;
    };

    if let Some(added_note) = mapping_value_mut(note, "note").and_then(Value::as_mapping_mut)
        && let Some(fields) = mapping_value_mut(added_note, "fields")
    {
        migrate_field_mapping(fields, media_path_lookup, report);
    }

    if let Some(fields) = mapping_value_mut(note, "fields") {
        migrate_field_mapping(fields, media_path_lookup, report);
    }
}

fn migrate_field_mapping(
    fields_value: &mut Value,
    media_path_lookup: &BTreeMap<String, Option<String>>,
    report: &mut ImageMigrationReport,
) {
    let Some(fields) = fields_value.as_mapping_mut() else {
        return;
    };
    for (_field_id, field_value) in fields {
        if let Some(field_change) = field_value.as_mapping_mut() {
            if let Some(value) = mapping_value_mut(field_change, "value") {
                migrate_one_field_value(value, media_path_lookup, report);
            }
        } else {
            migrate_one_field_value(field_value, media_path_lookup, report);
        }
    }
}

fn migrate_one_field_value(
    value: &mut Value,
    media_path_lookup: &BTreeMap<String, Option<String>>,
    report: &mut ImageMigrationReport,
) {
    let Some(text) = value.as_str() else {
        return;
    };
    let Some(paths) = media::strict_image_tag_paths(text) else {
        if text.trim().contains("<img") {
            report.skipped_non_strict += 1;
        }
        return;
    };

    let mut media_ids = Vec::new();
    let mut missing = false;
    let mut ambiguous = false;
    for path in paths {
        match media_path_lookup.get(&path) {
            Some(Some(media_id)) => media_ids.push(media_id.clone()),
            Some(None) => ambiguous = true,
            None => missing = true,
        }
    }

    if missing {
        report.skipped_no_match += 1;
        return;
    }
    if ambiguous {
        report.skipped_ambiguous_path += 1;
        return;
    }

    *value = image_reference_value(&media_ids);
    report.converted += 1;
}

fn image_reference_value(media_ids: &[String]) -> Value {
    match media_ids {
        [media_id] => tagged_image_value(media_id),
        _ => Value::Sequence(
            media_ids
                .iter()
                .map(|media_id| tagged_image_value(media_id))
                .collect(),
        ),
    }
}

fn tagged_image_value(media_id: &str) -> Value {
    serde_yaml::from_str(&format!("!image {media_id}"))
        .expect("stable media IDs are valid !image scalar values")
}

fn mapping_value_mut<'a>(mapping: &'a mut Mapping, key: &str) -> Option<&'a mut Value> {
    mapping.get_mut(Value::String(key.to_owned()))
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
