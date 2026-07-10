use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::safe_relative_path::SafeRelativePath;

use serde_yaml::Value;

use crate::{strict_yaml, yaml_scalar};

/// Resolve `!include path` tagged scalar authoring conveniences in a Canonical Deck or overlay YAML file.
///
/// Include paths use canonical portable safe-relative syntax and are interpreted relative to
/// `package_root`. `safe_include_roots` remains in the compatibility API, but cannot authorize
/// `.`/`..` syntax or a path outside the package-root-relative namespace.
pub fn resolve_file_includes(
    input: &str,
    source_path: &Path,
    package_root: &Path,
    safe_include_roots: &[PathBuf],
) -> Result<String, IncludeError> {
    strict_yaml::reject_duplicate_keys(input).map_err(|error| IncludeError {
        source_path: source_path.to_path_buf(),
        yaml_path: String::new(),
        include_path: String::new(),
        kind: Box::new(IncludeErrorKind::Parse(error.to_string())),
    })?;
    if !input.contains("!include") {
        return Ok(input.to_owned());
    }

    let mut value = serde_yaml::from_str::<Value>(input).map_err(|error| IncludeError {
        source_path: source_path.to_path_buf(),
        yaml_path: String::new(),
        include_path: String::new(),
        kind: Box::new(IncludeErrorKind::Parse(error.to_string())),
    })?;
    let mut resolver = IncludeResolver::new(source_path, package_root, safe_include_roots)?;
    resolver.resolve_value(&mut value, &mut Vec::new(), &mut Vec::new())?;
    serde_yaml::to_string(&value).map_err(|error| IncludeError {
        source_path: source_path.to_path_buf(),
        yaml_path: String::new(),
        include_path: String::new(),
        kind: Box::new(IncludeErrorKind::Parse(error.to_string())),
    })
}

/// Resolve an include target with the same package-root and safe-root checks used by file includes.
pub fn resolve_include_target(
    include_path: &str,
    package_root: &Path,
    safe_include_roots: &[PathBuf],
) -> Result<PathBuf, IncludeError> {
    let resolver = IncludeResolver::new(package_root, package_root, safe_include_roots)?;
    resolver.resolve_include_path(include_path, &[])
}

/// Format a raw YAML source file without materializing `!include` tagged scalars.
///
/// The domain formatter still owns canonical ordering and scalar emission. Include
/// tags are temporarily replaced with unique scalar sentinels before formatting,
/// then restored as `!include <path>` tagged scalars in the canonical output.
/// This keeps formatting and verification on one include-preserving path while
/// leaving file include resolution as a read-path concern.
pub fn format_preserving_file_includes<E>(
    input: &str,
    format: impl FnOnce(&str) -> Result<String, E>,
) -> Result<String, String>
where
    E: ToString,
{
    strict_yaml::reject_duplicate_keys(input).map_err(|error| error.to_string())?;
    if !input.contains("!include") {
        return format(input).map_err(|error| error.to_string());
    }

    let mut value = serde_yaml::from_str::<Value>(input).map_err(|error| error.to_string())?;
    // ADR-0016 §7 deliberately whitelists only top-level `media: !include` as a
    // structural include. The formatter strips that key so CanonicalDeckYaml's
    // default empty media map can parse, then restores the directive after emit.
    let media_include = strip_top_level_media_include(&mut value)?;
    let mut replacements = Vec::new();
    replace_includes_with_sentinels(input, &mut value, &mut replacements)?;
    let source_with_sentinels = serde_yaml::to_string(&value).map_err(|error| error.to_string())?;
    let mut formatted = format(&source_with_sentinels).map_err(|error| error.to_string())?;
    formatted = restore_top_level_media_include(formatted, media_include)?;
    for replacement in replacements {
        formatted = formatted.replace(&replacement.sentinel, &replacement.directive);
    }
    Ok(formatted)
}

struct IncludeReplacement {
    sentinel: String,
    directive: String,
}

struct MediaIncludeReplacement {
    directive: String,
}

fn strip_top_level_media_include(
    value: &mut Value,
) -> Result<Option<MediaIncludeReplacement>, String> {
    let image_ids = collect_structured_image_ids(value);
    let Value::Mapping(mapping) = value else {
        return Ok(None);
    };
    let media_key = Value::String("media".to_owned());
    let Some(media_value) = mapping.get(&media_key) else {
        return Ok(None);
    };
    let Value::Tagged(tagged) = media_value else {
        return Ok(None);
    };
    if tagged.tag != "include" {
        return Ok(None);
    }
    let Value::String(path) = &tagged.value else {
        return Err("!include path must be a scalar string".to_owned());
    };
    let directive = format!("media: !include {}", yaml_scalar::scalar(path));
    let synthetic_media = image_ids
        .into_iter()
        .map(|id| {
            let mut reference = serde_yaml::Mapping::new();
            reference.insert(
                Value::String("path".to_owned()),
                Value::String(format!("__brain_brew_media_{id}__")),
            );
            reference.insert(
                Value::String("sha256".to_owned()),
                Value::String(String::new()),
            );
            (Value::String(id), Value::Mapping(reference))
        })
        .collect();
    mapping.insert(media_key, Value::Mapping(synthetic_media));
    Ok(Some(MediaIncludeReplacement { directive }))
}

fn restore_top_level_media_include(
    formatted: String,
    media_include: Option<MediaIncludeReplacement>,
) -> Result<String, String> {
    let Some(media_include) = media_include else {
        return Ok(formatted);
    };
    let start = formatted.find("media:").ok_or_else(|| {
        format!(
            "missing top-level media section for `{}`",
            media_include.directive
        )
    })?;
    if start > 0 && !formatted[..start].ends_with('\n') {
        return Err("top-level media section was not line aligned".to_owned());
    }
    let end = formatted[start..]
        .find("tombstones:")
        .map(|offset| start + offset)
        .ok_or_else(|| "missing tombstones section after top-level media".to_owned())?;
    let mut restored = String::with_capacity(formatted.len() + media_include.directive.len());
    restored.push_str(&formatted[..start]);
    restored.push_str(&media_include.directive);
    restored.push('\n');
    restored.push_str(&formatted[end..]);
    Ok(restored)
}

fn collect_structured_image_ids(value: &Value) -> BTreeSet<String> {
    fn collect(value: &Value, ids: &mut BTreeSet<String>) {
        match value {
            Value::Tagged(tagged) if tagged.tag == "image" => {
                if let Value::String(id) = &tagged.value {
                    ids.insert(id.clone());
                }
            }
            Value::Tagged(tagged) => collect(&tagged.value, ids),
            Value::Sequence(sequence) => {
                for item in sequence {
                    collect(item, ids);
                }
            }
            Value::Mapping(mapping) => {
                for item in mapping.values() {
                    collect(item, ids);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    let mut ids = BTreeSet::new();
    collect(value, &mut ids);
    ids
}

fn replace_includes_with_sentinels(
    original_input: &str,
    value: &mut Value,
    replacements: &mut Vec<IncludeReplacement>,
) -> Result<(), String> {
    match value {
        Value::Tagged(tagged) if tagged.tag == "include" => {
            let Value::String(path) = &tagged.value else {
                return Err("!include path must be a scalar string".to_owned());
            };
            let sentinel = next_include_sentinel(original_input, replacements.len());
            replacements.push(IncludeReplacement {
                sentinel: sentinel.clone(),
                directive: format!("!include {}", yaml_scalar::scalar(path)),
            });
            *value = Value::String(sentinel);
        }
        Value::Tagged(tagged) => {
            replace_includes_with_sentinels(original_input, &mut tagged.value, replacements)?
        }
        Value::Sequence(sequence) => {
            for item in sequence {
                replace_includes_with_sentinels(original_input, item, replacements)?;
            }
        }
        Value::Mapping(mapping) => {
            for (_, item) in mapping {
                replace_includes_with_sentinels(original_input, item, replacements)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn next_include_sentinel(original_input: &str, index: usize) -> String {
    let mut attempt = 0;
    loop {
        let sentinel = format!("__brain_brew_include_{index}_{attempt}__");
        if !original_input.contains(&sentinel) {
            return sentinel;
        }
        attempt += 1;
    }
}

#[derive(Debug)]
pub struct IncludeError {
    source_path: PathBuf,
    yaml_path: String,
    include_path: String,
    kind: Box<IncludeErrorKind>,
}

#[derive(Debug)]
enum IncludeErrorKind {
    Parse(String),
    UnsupportedTag(String),
    IncludePathNotScalar,
    NotScalarContentField,
    UnsafeSyntax(String),
    EscapesPackageRoot {
        package_root: PathBuf,
    },
    Unreadable {
        resolved_path: PathBuf,
        message: String,
    },
    PackageRootUnreadable {
        package_root: PathBuf,
        message: String,
    },
    Cyclic {
        chain: Vec<String>,
    },
    InvalidNestedDirective(String),
    StructuralIncludeRootNotMapping {
        resolved_path: PathBuf,
        found: &'static str,
    },
}

impl IncludeError {
    fn new(
        source_path: &Path,
        yaml_path: &[String],
        include_path: impl Into<String>,
        kind: IncludeErrorKind,
    ) -> Self {
        Self {
            source_path: source_path.to_path_buf(),
            yaml_path: yaml_path_display(yaml_path),
            include_path: include_path.into(),
            kind: Box::new(kind),
        }
    }
}

impl fmt::Display for IncludeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let location = if self.yaml_path.is_empty() {
            self.source_path.display().to_string()
        } else {
            format!("{}:{}", self.source_path.display(), self.yaml_path)
        };
        match self.kind.as_ref() {
            IncludeErrorKind::Parse(error) => {
                write!(f, "{location}: failed to parse YAML for includes: {error}")
            }
            IncludeErrorKind::UnsupportedTag(tag) => write!(
                f,
                "{location}: unsupported YAML tag {tag}; only !include is supported for file includes"
            ),
            IncludeErrorKind::IncludePathNotScalar => {
                write!(f, "{location}: !include path must be a scalar string")
            }
            IncludeErrorKind::NotScalarContentField => write!(
                f,
                "{location}: !include {} is only valid for scalar content fields",
                self.include_path
            ),
            IncludeErrorKind::UnsafeSyntax(reason) => write!(
                f,
                "{location}: include path {} is not a safe package-root-relative path: {reason}",
                self.include_path
            ),
            IncludeErrorKind::EscapesPackageRoot { package_root } => write!(
                f,
                "{location}: include path {} escapes package root {}; configure include_roots to allow a safe external root",
                self.include_path,
                package_root.display()
            ),
            IncludeErrorKind::Unreadable {
                resolved_path,
                message,
            } => write!(
                f,
                "{location}: include path {} ({}) could not be read: {message}",
                self.include_path,
                resolved_path.display()
            ),
            IncludeErrorKind::PackageRootUnreadable {
                package_root,
                message,
            } => write!(
                f,
                "{location}: package root {} could not be read: {message}",
                package_root.display()
            ),
            IncludeErrorKind::Cyclic { chain } => write!(
                f,
                "{location}: cyclic include while reading {}: {}",
                self.include_path,
                chain.join(" -> ")
            ),
            IncludeErrorKind::InvalidNestedDirective(message) => write!(
                f,
                "{location}: invalid nested include directive in {}: {message}",
                self.include_path
            ),
            IncludeErrorKind::StructuralIncludeRootNotMapping {
                resolved_path,
                found,
            } => write!(
                f,
                "{location}: structural media include {} ({}) must have a mapping root, found {found}",
                self.include_path,
                resolved_path.display()
            ),
        }
    }
}

impl std::error::Error for IncludeError {}

enum StructuralIncludeKind {
    MediaMap,
}

fn structural_include_kind(path: &[String]) -> Option<StructuralIncludeKind> {
    // ADR-0016 §7 makes this a deliberate whitelist, not general YAML AST splicing.
    match path {
        [segment] if segment == "media" => Some(StructuralIncludeKind::MediaMap),
        _ => None,
    }
}

struct IncludeResolver {
    source_path: PathBuf,
    package_root: PathBuf,
    allowed_roots: Vec<PathBuf>,
}

impl IncludeResolver {
    fn new(
        source_path: &Path,
        package_root: &Path,
        safe_include_roots: &[PathBuf],
    ) -> Result<Self, IncludeError> {
        let package_root = fs::canonicalize(package_root).map_err(|error| IncludeError {
            source_path: source_path.to_path_buf(),
            yaml_path: String::new(),
            include_path: package_root.display().to_string(),
            kind: Box::new(IncludeErrorKind::PackageRootUnreadable {
                package_root: package_root.to_path_buf(),
                message: error.to_string(),
            }),
        })?;
        let mut allowed_roots = vec![package_root.clone()];
        for root in safe_include_roots {
            if let Ok(root) = fs::canonicalize(root) {
                allowed_roots.push(root);
            }
        }
        Ok(Self {
            source_path: source_path.to_path_buf(),
            package_root,
            allowed_roots,
        })
    }

    fn resolve_value(
        &mut self,
        value: &mut Value,
        yaml_path: &mut Vec<String>,
        include_stack: &mut Vec<IncludeStackEntry>,
    ) -> Result<(), IncludeError> {
        match value {
            Value::Tagged(tagged) if tagged.tag == "include" => {
                let include_path = match &tagged.value {
                    Value::String(path) => path.clone(),
                    _ => {
                        return Err(IncludeError::new(
                            &self.source_path,
                            yaml_path,
                            "",
                            IncludeErrorKind::IncludePathNotScalar,
                        ));
                    }
                };
                if let Some(StructuralIncludeKind::MediaMap) = structural_include_kind(yaml_path) {
                    let included = self.read_media_map_include(&include_path, yaml_path)?;
                    *value = included;
                    return Ok(());
                }
                if !is_scalar_content_path(yaml_path) {
                    return Err(IncludeError::new(
                        &self.source_path,
                        yaml_path,
                        include_path,
                        IncludeErrorKind::NotScalarContentField,
                    ));
                }
                let included = self.read_include(&include_path, yaml_path, include_stack)?;
                *value = Value::String(included);
            }
            Value::Tagged(tagged) if tagged.tag == "image" => {}
            Value::Tagged(tagged) => {
                return Err(IncludeError::new(
                    &self.source_path,
                    yaml_path,
                    "",
                    IncludeErrorKind::UnsupportedTag(tagged.tag.to_string()),
                ));
            }
            Value::Mapping(mapping) => {
                for (key, value) in mapping.iter_mut() {
                    yaml_path.push(path_segment(key));
                    self.resolve_value(value, yaml_path, include_stack)?;
                    yaml_path.pop();
                }
            }
            Value::Sequence(sequence) => {
                for (index, item) in sequence.iter_mut().enumerate() {
                    yaml_path.push(format!("[{index}]"));
                    self.resolve_value(item, yaml_path, include_stack)?;
                    yaml_path.pop();
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
        Ok(())
    }

    fn read_media_map_include(
        &self,
        include_path: &str,
        yaml_path: &[String],
    ) -> Result<Value, IncludeError> {
        let resolved = self.resolve_include_path(include_path, yaml_path)?;
        let content = fs::read_to_string(&resolved).map_err(|error| {
            IncludeError::new(
                &self.source_path,
                yaml_path,
                include_path,
                IncludeErrorKind::Unreadable {
                    resolved_path: resolved.clone(),
                    message: error.to_string(),
                },
            )
        })?;
        strict_yaml::reject_duplicate_keys(&content).map_err(|error| IncludeError {
            source_path: resolved.clone(),
            yaml_path: String::new(),
            include_path: include_path.to_owned(),
            kind: Box::new(IncludeErrorKind::Parse(error.to_string())),
        })?;
        let value = serde_yaml::from_str::<Value>(&content).map_err(|error| IncludeError {
            source_path: resolved.clone(),
            yaml_path: String::new(),
            include_path: include_path.to_owned(),
            kind: Box::new(IncludeErrorKind::Parse(error.to_string())),
        })?;
        reject_yaml_tags_in_included_media(&value, &mut Vec::new(), &resolved, include_path)?;
        match value {
            Value::Mapping(_) => Ok(value),
            other => Err(IncludeError::new(
                &self.source_path,
                yaml_path,
                include_path,
                IncludeErrorKind::StructuralIncludeRootNotMapping {
                    resolved_path: resolved,
                    found: value_kind(&other),
                },
            )),
        }
    }

    fn read_include(
        &mut self,
        include_path: &str,
        yaml_path: &[String],
        include_stack: &mut Vec<IncludeStackEntry>,
    ) -> Result<String, IncludeError> {
        let resolved = self.resolve_include_path(include_path, yaml_path)?;
        if let Some(cycle_start) = include_stack
            .iter()
            .position(|entry| entry.resolved_path == resolved)
        {
            let mut chain = include_stack[cycle_start..]
                .iter()
                .map(|entry| entry.include_path.clone())
                .collect::<Vec<_>>();
            chain.push(include_path.to_owned());
            return Err(IncludeError::new(
                &self.source_path,
                yaml_path,
                include_path,
                IncludeErrorKind::Cyclic { chain },
            ));
        }

        let content = fs::read_to_string(&resolved).map_err(|error| {
            IncludeError::new(
                &self.source_path,
                yaml_path,
                include_path,
                IncludeErrorKind::Unreadable {
                    resolved_path: resolved.clone(),
                    message: error.to_string(),
                },
            )
        })?;

        if let Some(nested_path) = nested_include_directive(&content).map_err(|message| {
            IncludeError::new(
                &self.source_path,
                yaml_path,
                include_path,
                IncludeErrorKind::InvalidNestedDirective(message),
            )
        })? {
            include_stack.push(IncludeStackEntry {
                include_path: include_path.to_owned(),
                resolved_path: resolved,
            });
            let nested = self.read_include(&nested_path, yaml_path, include_stack);
            include_stack.pop();
            nested
        } else {
            Ok(content)
        }
    }

    fn resolve_include_path(
        &self,
        include_path: &str,
        yaml_path: &[String],
    ) -> Result<PathBuf, IncludeError> {
        let requested = SafeRelativePath::new(include_path).map_err(|error| {
            IncludeError::new(
                &self.source_path,
                yaml_path,
                include_path,
                IncludeErrorKind::UnsafeSyntax(error.to_string()),
            )
        })?;
        let joined = self.package_root.join(requested.as_path());
        if let Ok(canonical) = fs::canonicalize(&joined)
            && !self.is_under_allowed_root(&canonical)
        {
            return Err(IncludeError::new(
                &self.source_path,
                yaml_path,
                include_path,
                IncludeErrorKind::EscapesPackageRoot {
                    package_root: self.package_root.clone(),
                },
            ));
        }
        Ok(joined)
    }

    fn is_under_allowed_root(&self, path: &Path) -> bool {
        self.allowed_roots.iter().any(|root| path.starts_with(root))
    }
}

#[derive(Clone)]
struct IncludeStackEntry {
    include_path: String,
    resolved_path: PathBuf,
}

fn reject_yaml_tags_in_included_media(
    value: &Value,
    yaml_path: &mut Vec<String>,
    source_path: &Path,
    include_path: &str,
) -> Result<(), IncludeError> {
    match value {
        Value::Tagged(tagged) => Err(IncludeError::new(
            source_path,
            yaml_path,
            include_path,
            IncludeErrorKind::UnsupportedTag(tagged.tag.to_string()),
        )),
        Value::Mapping(mapping) => {
            for (key, item) in mapping {
                reject_yaml_tags_in_included_media(key, yaml_path, source_path, include_path)?;
                yaml_path.push(path_segment(key));
                reject_yaml_tags_in_included_media(item, yaml_path, source_path, include_path)?;
                yaml_path.pop();
            }
            Ok(())
        }
        Value::Sequence(sequence) => {
            for (index, item) in sequence.iter().enumerate() {
                yaml_path.push(format!("[{index}]"));
                reject_yaml_tags_in_included_media(item, yaml_path, source_path, include_path)?;
                yaml_path.pop();
            }
            Ok(())
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Ok(()),
    }
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "scalar",
        Value::Sequence(_) => "sequence",
        Value::Mapping(_) => "mapping",
        Value::Tagged(_) => "tagged value",
    }
}

fn nested_include_directive(content: &str) -> Result<Option<String>, String> {
    let trimmed = content.trim();
    if trimmed != "!include" && !trimmed.starts_with("!include ") {
        return Ok(None);
    }
    let value = serde_yaml::from_str::<Value>(trimmed).map_err(|error| error.to_string())?;
    match value {
        Value::Tagged(tagged) if tagged.tag == "include" => match tagged.value {
            Value::String(path) => Ok(Some(path)),
            _ => Err("!include path must be a scalar string".to_owned()),
        },
        _ => Ok(None),
    }
}

pub(crate) fn is_scalar_content_path(path: &[String]) -> bool {
    let Some(last) = path.last().map(String::as_str) else {
        return false;
    };
    if path
        .iter()
        .any(|segment| segment == "adapter_ids" || segment == "media")
    {
        return false;
    }
    if matches!(
        last,
        "id" | "kind" | "intent" | "note_type_id" | "insert_after" | "path" | "sha256"
    ) {
        return false;
    }
    if matches!(
        last,
        "name" | "description" | "question_format" | "answer_format" | "styling"
    ) {
        return true;
    }
    if matches!(last, "text" | "literal" | "format")
        && (path.iter().any(|segment| segment == "message")
            || path.iter().any(|segment| segment == "fields")
            || path.iter().any(|segment| segment == "field_fills"))
    {
        return true;
    }
    if path.iter().any(|segment| segment == "variables") && last != "variables" {
        return true;
    }
    if path
        .first()
        .is_some_and(|segment| segment == "translations")
        && path.get(1).is_none_or(|segment| segment != "adapter_ids")
        && path.len() >= 3
    {
        return true;
    }
    if path.first().is_some_and(|segment| segment == "field_fills") && path.len() == 3 {
        return true;
    }
    if path
        .first()
        .is_some_and(|segment| segment == "field_additions")
    {
        return (path.len() == 4 && path.get(2).is_some_and(|segment| segment == "fields"))
            || (path.len() == 5 && path.get(2).is_some_and(|segment| segment == "values"));
    }
    if path.len() == 4
        && path.first().is_some_and(|segment| segment == "notes")
        && path.get(2).is_some_and(|segment| segment == "fields")
    {
        return true;
    }
    if path.len() == 5
        && path.first().is_some_and(|segment| segment == "notes")
        && path.get(2).is_some_and(|segment| segment == "note")
        && path.get(3).is_some_and(|segment| segment == "fields")
    {
        return true;
    }
    last == "value"
}

fn path_segment(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => "null".to_owned(),
        Value::Tagged(tagged) => format!("!{}", tagged.tag),
        Value::Sequence(_) => "[]".to_owned(),
        Value::Mapping(_) => "{}".to_owned(),
    }
}

fn yaml_path_display(path: &[String]) -> String {
    let mut display = String::new();
    for segment in path {
        if segment.starts_with('[') {
            display.push_str(segment);
        } else {
            if !display.is_empty() {
                display.push('.');
            }
            display.push_str(segment);
        }
    }
    display
}
