use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use brain_brew_core::{
    CanonicalDeck, Overlay, TranslationCoverageCategory, TranslationCoverageEntry,
    TranslationCoverageReport,
};
use serde_json::json;

use crate::help;
use crate::io::{PlannedOverlay, plan_manifest_target_with_packages, read_manifest};
use crate::output;

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        print!(
            "{}",
            help::command("translations").expect("translations help exists")
        );
        return Ok(());
    }
    let args = parse_translation_args(args)?;
    let manifest = read_manifest(&args.manifest_path)?;
    let target_names = if args.all_targets || args.target.is_none() {
        manifest.targets.keys().cloned().collect::<Vec<_>>()
    } else {
        vec![args.target.clone().expect("target exists")]
    };

    let mut reports = Vec::new();
    let mut stubs_by_file = BTreeMap::<PathBuf, BTreeSet<String>>::new();
    for target in &target_names {
        let plan = plan_manifest_target_with_packages(
            &args.manifest_path,
            target,
            &args.include_paths,
            &args.package_roots,
        )?;
        let mut current = plan.base.clone();
        for (planned, overlay) in &plan.overlays {
            if overlay.translations.is_some() {
                if overlay_matches_scope(target, planned, overlay, &args)
                    && let Some(report) = current.translation_coverage(overlay)
                {
                    let entries = report
                        .entries
                        .into_iter()
                        .filter(|entry| entry_matches_scope(entry, &args))
                        .collect::<Vec<_>>();
                    if args.apply {
                        let stubs = entries
                            .iter()
                            .filter(|entry| {
                                entry.category == TranslationCoverageCategory::UntranslatedFallback
                            })
                            .map(|entry| entry.source.clone())
                            .collect::<BTreeSet<_>>();
                        if !stubs.is_empty() {
                            stubs_by_file
                                .entry(planned.file.clone())
                                .or_default()
                                .extend(stubs);
                        }
                    }
                    reports.push(ScopedTranslationReport {
                        target: target.clone(),
                        overlay_id: planned.id.clone(),
                        overlay_file: planned.display_file.clone(),
                        report: TranslationCoverageReport {
                            overlay_id: report.overlay_id,
                            entries,
                        },
                    });
                }
                current = compose_lenient_translation_overlay(&current, overlay)?;
            } else {
                current = current
                    .compose(std::slice::from_ref(overlay))
                    .map_err(|error| {
                        format!(
                            "failed to compose overlay {} for target {target}: {error}",
                            planned.id
                        )
                    })?;
            }
        }
    }

    let mut applied = BTreeMap::new();
    if args.apply {
        for (path, stubs) in &stubs_by_file {
            let count = apply_direct_stubs(path, stubs)?;
            if count > 0 {
                applied.insert(path.display().to_string(), count);
            }
        }
    }

    if args.json_output {
        print_json_reports(&reports, &applied);
    } else {
        print_human_reports(&reports);
        if args.apply {
            let total = applied.values().sum::<usize>();
            let details = applied
                .iter()
                .map(|(file, count)| (file.as_str(), count.to_string()))
                .collect::<Vec<_>>();
            output::print_success(format!("applied {total} translation stub(s)"), &details);
        }
    }

    Ok(())
}

struct TranslationArgs {
    manifest_path: PathBuf,
    target: Option<String>,
    all_targets: bool,
    include_paths: Vec<PathBuf>,
    package_roots: Vec<PathBuf>,
    language: Option<String>,
    overlay: Option<String>,
    note: Option<String>,
    field: Option<String>,
    path_prefixes: Vec<String>,
    apply: bool,
    json_output: bool,
}

fn parse_translation_args(args: &[String]) -> Result<TranslationArgs, String> {
    let mut parsed = TranslationArgs {
        manifest_path: PathBuf::from("brainbrew.yaml"),
        target: None,
        all_targets: false,
        include_paths: Vec::new(),
        package_roots: Vec::new(),
        language: None,
        overlay: None,
        note: None,
        field: None,
        path_prefixes: Vec::new(),
        apply: false,
        json_output: false,
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
            "--target" => {
                let Some(name) = args.get(index + 1) else {
                    return Err("--target requires a name".to_owned());
                };
                parsed.target = Some(name.clone());
                index += 2;
            }
            "--all-targets" => {
                parsed.all_targets = true;
                index += 1;
            }
            "--include" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--include requires a path".to_owned());
                };
                parsed.include_paths.push(PathBuf::from(path));
                index += 2;
            }
            "--package-root" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--package-root requires a path".to_owned());
                };
                parsed.package_roots.push(PathBuf::from(path));
                index += 2;
            }
            "--language" => {
                let Some(language) = args.get(index + 1) else {
                    return Err("--language requires a code".to_owned());
                };
                parsed.language = Some(language.clone());
                index += 2;
            }
            "--overlay" => {
                let Some(overlay) = args.get(index + 1) else {
                    return Err("--overlay requires an id or file fragment".to_owned());
                };
                parsed.overlay = Some(overlay.clone());
                index += 2;
            }
            "--note" => {
                let Some(note) = args.get(index + 1) else {
                    return Err("--note requires a note id".to_owned());
                };
                parsed.note = Some(note.clone());
                index += 2;
            }
            "--field" => {
                let Some(field) = args.get(index + 1) else {
                    return Err("--field requires a field id".to_owned());
                };
                parsed.field = Some(field.clone());
                index += 2;
            }
            "--path-prefix" => {
                let Some(prefix) = args.get(index + 1) else {
                    return Err("--path-prefix requires a deck path prefix".to_owned());
                };
                parsed.path_prefixes.push(prefix.clone());
                index += 2;
            }
            "--apply" => {
                parsed.apply = true;
                index += 1;
            }
            "--json" => {
                parsed.json_output = true;
                index += 1;
            }
            other => return Err(format!("unexpected translations argument {other:?}")),
        }
    }
    if parsed.all_targets && parsed.target.is_some() {
        return Err("choose --all-targets or --target, not both".to_owned());
    }
    Ok(parsed)
}

struct ScopedTranslationReport {
    target: String,
    overlay_id: String,
    overlay_file: String,
    report: TranslationCoverageReport,
}

fn overlay_matches_scope(
    target: &str,
    planned: &PlannedOverlay,
    overlay: &Overlay,
    args: &TranslationArgs,
) -> bool {
    if let Some(overlay_filter) = &args.overlay {
        let overlay_id = overlay.id.as_str();
        if !planned.id.contains(overlay_filter)
            && !planned.display_file.contains(overlay_filter)
            && !overlay_id.contains(overlay_filter)
        {
            return false;
        }
    }
    if let Some(language) = &args.language
        && !language_matches(language, target, planned, overlay)
    {
        return false;
    }
    true
}

fn language_matches(
    language: &str,
    target: &str,
    planned: &PlannedOverlay,
    overlay: &Overlay,
) -> bool {
    let language = language.trim();
    if language.is_empty() {
        return true;
    }
    let target_has_language = target == language
        || target.starts_with(&format!("{language}-"))
        || target.ends_with(&format!("-{language}"));
    let overlay_id = overlay.id.as_str();
    let file_stem = Path::new(&planned.display_file)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    target_has_language
        || planned.id.contains(&format!(".{language}"))
        || planned.id.contains(&format!("-{language}"))
        || overlay_id.contains(&format!(".{language}"))
        || overlay_id.contains(&format!("-{language}"))
        || file_stem == language
        || planned
            .display_file
            .split(['/', '\\'])
            .any(|part| part == language || part == format!("{language}.yaml"))
}

fn entry_matches_scope(entry: &TranslationCoverageEntry, args: &TranslationArgs) -> bool {
    if let Some(note) = &args.note {
        let full = if note.starts_with("note.") {
            note.clone()
        } else {
            format!("note.{note}")
        };
        if !entry.path.starts_with(&format!("notes.{note}."))
            && !entry.path.starts_with(&format!("notes.{full}."))
            && !entry.path.contains(&format!(".notes.{note}."))
            && !entry.path.contains(&format!(".notes.{full}."))
        {
            return false;
        }
    }
    if let Some(field) = &args.field {
        let full = if field.starts_with("field.") {
            field.clone()
        } else {
            format!("field.{field}")
        };
        if !entry.path.contains(&format!(".fields.{field}"))
            && !entry.path.contains(&format!(".fields.{full}"))
        {
            return false;
        }
    }
    if !args.path_prefixes.is_empty()
        && !args
            .path_prefixes
            .iter()
            .any(|prefix| entry.path == *prefix || entry.path.starts_with(&format!("{prefix}.")))
    {
        return false;
    }
    true
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

fn print_human_reports(reports: &[ScopedTranslationReport]) {
    for report in reports {
        let counts = category_counts(&report.report.entries);
        println!(
            "Translation coverage for target {} overlay {} ({})",
            report.target, report.overlay_id, report.overlay_file
        );
        println!(
            "  direct translations: {}",
            counts.get("direct_translation").copied().unwrap_or(0)
        );
        println!(
            "  contextual overrides: {}",
            counts.get("contextual_override").copied().unwrap_or(0)
        );
        println!(
            "  target-language additions: {}",
            counts.get("target_language_addition").copied().unwrap_or(0)
        );
        println!(
            "  stale/invalid keys: {}",
            report
                .report
                .entries
                .iter()
                .filter(|entry| {
                    matches!(
                        entry.category,
                        TranslationCoverageCategory::StaleDirectKey
                            | TranslationCoverageCategory::StaleContextualKey
                            | TranslationCoverageCategory::StaleTargetAddition
                            | TranslationCoverageCategory::StaleVariableKey
                            | TranslationCoverageCategory::StaleAdapterIdKey
                            | TranslationCoverageCategory::InvalidTargetAddition
                    )
                })
                .count()
        );
        println!(
            "  missing/untranslated fallbacks: {}",
            counts.get("untranslated_fallback").copied().unwrap_or(0)
        );
        for entry in report.report.problem_entries() {
            println!(
                "  - {} {} source={} translated={}",
                entry.category.as_str(),
                entry.path,
                yaml_scalar(&entry.source),
                entry
                    .translated
                    .as_deref()
                    .map(yaml_scalar)
                    .unwrap_or_else(|| "''".to_owned())
            );
        }
    }
}

fn print_json_reports(reports: &[ScopedTranslationReport], applied: &BTreeMap<String, usize>) {
    let reports_json = reports
        .iter()
        .map(|report| {
            json!({
                "target": report.target,
                "overlay": report.overlay_id,
                "file": report.overlay_file,
                "summary": category_counts(&report.report.entries),
                "entries": report.report.entries.iter().map(entry_json).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "reports": reports_json,
            "applied": applied,
        }))
        .expect("translation report JSON serializes")
    );
}

fn entry_json(entry: &TranslationCoverageEntry) -> serde_json::Value {
    json!({
        "category": entry.category.as_str(),
        "path": entry.path,
        "source": entry.source,
        "translated": entry.translated,
        "context": entry.context,
    })
}

fn category_counts(entries: &[TranslationCoverageEntry]) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry.category.as_str()).or_insert(0) += 1;
    }
    counts
}

fn apply_direct_stubs(path: &Path, stubs: &BTreeSet<String>) -> Result<usize, String> {
    if stubs.is_empty() {
        return Ok(0);
    }
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let stub_lines = stubs
        .iter()
        .map(|source| format!("    {}: {}\n", yaml_scalar(source), yaml_scalar(source)))
        .collect::<String>();
    let output = insert_direct_stub_lines(&input, &stub_lines);
    if output != input {
        fs::write(path, output).map_err(|error| format!("{}: {error}", path.display()))?;
    }
    Ok(stubs.len())
}

fn insert_direct_stub_lines(input: &str, stub_lines: &str) -> String {
    let spans = line_spans(input);
    let translations_line = spans.iter().position(|(start, end)| {
        let line = &input[*start..*end];
        line.trim_end() == "translations:"
    });

    if let Some(translations_index) = translations_line {
        let direct_index = spans
            .iter()
            .enumerate()
            .skip(translations_index + 1)
            .take_while(|(_, (start, end))| {
                let line = &input[*start..*end];
                line.trim().is_empty()
                    || line.trim_start().starts_with('#')
                    || line.starts_with(' ')
            })
            .find(|(_, (start, end))| input[*start..*end].trim_end() == "  direct:")
            .map(|(index, _)| index);
        if let Some(direct_index) = direct_index {
            let insert_at = spans
                .iter()
                .enumerate()
                .skip(direct_index + 1)
                .find(|(_, (start, end))| {
                    let line = &input[*start..*end];
                    !line.trim().is_empty()
                        && !line.trim_start().starts_with('#')
                        && (line.starts_with("  ") && !line.starts_with("    ")
                            || !line.starts_with(' '))
                })
                .map(|(_, (start, _))| *start)
                .unwrap_or(input.len());
            return insert_at_byte(input, insert_at, stub_lines);
        }

        let insert_at = spans[translations_index].1;
        return insert_at_byte(input, insert_at, &format!("  direct:\n{stub_lines}"));
    }

    let insert_after = spans
        .iter()
        .find(|(start, end)| input[*start..*end].trim_start().starts_with("kind:"))
        .map(|(_, end)| *end)
        .or_else(|| spans.first().map(|(_, end)| *end))
        .unwrap_or(0);
    insert_at_byte(
        input,
        insert_after,
        &format!("translations:\n  direct:\n{stub_lines}"),
    )
}

fn insert_at_byte(input: &str, index: usize, insertion: &str) -> String {
    let mut output = String::with_capacity(input.len() + insertion.len() + 1);
    output.push_str(&input[..index]);
    if index > 0 && !input[..index].ends_with('\n') {
        output.push('\n');
    }
    output.push_str(insertion);
    output.push_str(&input[index..]);
    output
}

fn line_spans(input: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = 0;
    for (index, ch) in input.char_indices() {
        if ch == '\n' {
            spans.push((start, index + 1));
            start = index + 1;
        }
    }
    if start < input.len() || input.is_empty() {
        spans.push((start, input.len()));
    }
    spans
}

fn yaml_scalar(value: &str) -> String {
    if !value.is_empty()
        && !value.starts_with([
            ' ', '-', '?', ':', '@', '`', '&', '*', '!', '|', '>', '#', '{', '[', ',',
        ])
        && !value.ends_with(' ')
        && value.chars().all(|ch| {
            ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '.' | ',' | '_' | '-' | '/' | ':')
        })
        && !value.chars().all(|ch| ch.is_ascii_digit())
        && !matches!(
            value,
            "true" | "false" | "True" | "False" | "TRUE" | "FALSE" | "null" | "Null" | "NULL"
        )
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}
