use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_yaml::Value;

use crate::yaml_scalar;

/// Resolve `!include path` tagged scalar authoring conveniences in a Canonical Deck or overlay YAML file.
///
/// Include paths are interpreted relative to `package_root`. Paths may not escape that root unless the
/// normalized target is under one of `safe_include_roots`.
pub fn resolve_file_includes(
    input: &str,
    source_path: &Path,
    package_root: &Path,
    safe_include_roots: &[PathBuf],
) -> Result<String, IncludeError> {
    if !input.contains("!include") {
        return Ok(input.to_owned());
    }

    let mut value = serde_yaml::from_str::<Value>(input).map_err(|error| IncludeError {
        source_path: source_path.to_path_buf(),
        yaml_path: String::new(),
        include_path: String::new(),
        kind: IncludeErrorKind::Parse(error.to_string()),
    })?;
    let mut resolver = IncludeResolver::new(source_path, package_root, safe_include_roots)?;
    resolver.resolve_value(&mut value, &mut Vec::new(), &mut Vec::new())?;
    serde_yaml::to_string(&value).map_err(|error| IncludeError {
        source_path: source_path.to_path_buf(),
        yaml_path: String::new(),
        include_path: String::new(),
        kind: IncludeErrorKind::Parse(error.to_string()),
    })
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
    if !input.contains("!include") {
        return format(input).map_err(|error| error.to_string());
    }

    let mut value = serde_yaml::from_str::<Value>(input).map_err(|error| error.to_string())?;
    let mut replacements = Vec::new();
    replace_includes_with_sentinels(input, &mut value, &mut replacements)?;
    let source_with_sentinels = serde_yaml::to_string(&value).map_err(|error| error.to_string())?;
    let mut formatted = format(&source_with_sentinels).map_err(|error| error.to_string())?;
    for replacement in replacements {
        formatted = formatted.replace(&replacement.sentinel, &replacement.directive);
    }
    Ok(formatted)
}

struct IncludeReplacement {
    sentinel: String,
    directive: String,
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
    kind: IncludeErrorKind,
}

#[derive(Debug)]
enum IncludeErrorKind {
    Parse(String),
    UnsupportedTag(String),
    IncludePathNotScalar,
    NotScalarContentField,
    AbsolutePath,
    EscapesPackageRoot {
        package_root: PathBuf,
    },
    Unreadable {
        resolved_path: PathBuf,
        message: String,
    },
    Cyclic {
        chain: Vec<String>,
    },
    InvalidNestedDirective(String),
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
            kind,
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
        match &self.kind {
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
            IncludeErrorKind::AbsolutePath => write!(
                f,
                "{location}: include path {} must be package-root-relative, not absolute",
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
        }
    }
}

impl std::error::Error for IncludeError {}

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
            include_path: String::new(),
            kind: IncludeErrorKind::Unreadable {
                resolved_path: package_root.to_path_buf(),
                message: error.to_string(),
            },
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
        let requested = Path::new(include_path);
        if requested.is_absolute() {
            return Err(IncludeError::new(
                &self.source_path,
                yaml_path,
                include_path,
                IncludeErrorKind::AbsolutePath,
            ));
        }
        let normalized = normalize_path(&self.package_root.join(requested));
        if !self.is_under_allowed_root(&normalized) {
            return Err(IncludeError::new(
                &self.source_path,
                yaml_path,
                include_path,
                IncludeErrorKind::EscapesPackageRoot {
                    package_root: self.package_root.clone(),
                },
            ));
        }
        if let Ok(canonical) = fs::canonicalize(&normalized)
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
        Ok(normalized)
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

fn is_scalar_content_path(path: &[String]) -> bool {
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

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}
