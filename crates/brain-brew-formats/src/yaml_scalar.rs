use std::fmt::Write as _;

/// Emit one YAML scalar with the same rules for every hand-rolled emitter.
///
/// Plain scalars are intentionally conservative: `:` is allowed only where YAML
/// cannot treat it as a mapping delimiter (`http://example` is plain, `key: value`
/// and `label:` are quoted), and a serde_yaml probe must parse the candidate back
/// as the same string. Everything else is quoted losslessly.
pub(crate) fn scalar(value: &str) -> String {
    if can_emit_plain_scalar(value) {
        value.to_owned()
    } else if needs_double_quoted_scalar(value) {
        double_quoted_scalar(value)
    } else {
        single_quoted_scalar(value)
    }
}

/// Write `key: value` using a block scalar for multiline values and `scalar` otherwise.
///
/// An explicit `2` indentation indicator is added when YAML auto-detection would
/// corrupt a value whose first content line starts with whitespace. Chomp
/// indicators preserve the exact trailing-newline shape.
pub(crate) fn write_multiline_or_scalar(out: &mut String, indent: &str, key: &str, value: &str) {
    if value.contains('\n') && !value.starts_with('\n') {
        let chomp = match trailing_newline_count(value) {
            0 => "-",
            1 => "",
            _ => "+",
        };
        let indentation = if needs_explicit_block_indent(value) {
            "2"
        } else {
            ""
        };
        writeln!(out, "{indent}{key}: |{indentation}{chomp}")
            .expect("writing to a string cannot fail");
        for line in block_content_lines(value) {
            writeln!(out, "{indent}  {line}").expect("writing to a string cannot fail");
        }
    } else {
        writeln!(out, "{indent}{key}: {}", scalar(value)).expect("writing to a string cannot fail");
    }
}

fn can_emit_plain_scalar(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with([
            ' ', '-', '?', ':', '@', '`', '&', '*', '!', '|', '>', '#', '{', '[', ',', '\t',
        ])
        && !value.ends_with([' ', '\t', ':'])
        && !contains_colon_mapping_indicator(value)
        && value.chars().all(is_allowed_plain_char)
        && parses_as_same_string(value)
}

fn is_allowed_plain_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '.' | ',' | '_' | '-' | '/' | ':')
}

fn contains_colon_mapping_indicator(value: &str) -> bool {
    value
        .as_bytes()
        .windows(2)
        .any(|window| window[0] == b':' && matches!(window[1], b' ' | b'\t'))
}

fn parses_as_same_string(value: &str) -> bool {
    let input = format!("value: {value}\n");
    let Ok(serde_yaml::Value::Mapping(mapping)) = serde_yaml::from_str::<serde_yaml::Value>(&input)
    else {
        return false;
    };
    mapping
        .get(serde_yaml::Value::String("value".to_owned()))
        .is_some_and(|parsed| parsed == &serde_yaml::Value::String(value.to_owned()))
}

fn needs_double_quoted_scalar(value: &str) -> bool {
    value
        .chars()
        .any(|ch| matches!(ch, '\0'..='\u{1f}' | '\u{7f}'))
}

fn single_quoted_scalar(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn double_quoted_scalar(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\0' => out.push_str("\\0"),
            '\u{07}' => out.push_str("\\a"),
            '\u{08}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{0b}' => out.push_str("\\v"),
            '\u{0c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            '\u{1b}' => out.push_str("\\e"),
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{01}'..='\u{06}' | '\u{0e}'..='\u{1a}' | '\u{1c}'..='\u{1f}' | '\u{7f}' => {
                write!(out, "\\x{:02X}", ch as u32).expect("writing to a string cannot fail");
            }
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn needs_explicit_block_indent(value: &str) -> bool {
    value.starts_with([' ', '\t'])
}

fn trailing_newline_count(value: &str) -> usize {
    value
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'\n')
        .count()
}

fn block_content_lines(value: &str) -> Vec<&str> {
    let mut lines = value.split('\n').collect::<Vec<_>>();
    if value.ends_with('\n') {
        lines.pop();
    }
    lines
}
