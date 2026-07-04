use std::fmt;

use crate::*;

/// Lightweight validation for Anki-shipped HTML/CSS content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContentKind {
    HtmlFragment,
    Css,
}

/// A report of structural content validation errors.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContentValidationReport {
    pub errors: Vec<ContentValidationError>,
}

impl ContentValidationReport {
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn push(&mut self, error: ContentValidationError) {
        self.errors.push(error);
    }

    pub fn extend(&mut self, other: ContentValidationReport) {
        self.errors.extend(other.errors);
    }
}

impl fmt::Display for ContentValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.errors.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ContentValidationReport {}

/// One structural content validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentValidationError {
    pub path: String,
    pub kind: ContentKind,
    pub line: Option<usize>,
    pub message: String,
}

impl ContentValidationError {
    pub fn new(
        path: impl Into<String>,
        kind: ContentKind,
        line: Option<usize>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            kind,
            line,
            message: message.into(),
        }
    }
}

impl fmt::Display for ContentValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "{}:{line}: {}", self.path, self.message),
            None => write!(f, "{}: {}", self.path, self.message),
        }
    }
}

/// Validate one HTML fragment or CSS stylesheet string without filesystem access.
pub fn validate_content_str(
    kind: ContentKind,
    path: impl Into<String>,
    source: &str,
) -> ContentValidationReport {
    let path = path.into();
    match kind {
        ContentKind::HtmlFragment => validate_html_fragment(&path, source),
        ContentKind::Css => validate_css(&path, source),
    }
}

/// Validate the Anki-shipped content surfaces of a composed deck.
///
/// This checks deck descriptions and card template question/answer formats as HTML
/// fragments, and note type styling as CSS. Callers that export to Anki should pass
/// a composed, variable-rendered deck so includes, overlays, translations, and
/// `${variable}` references are validated in the form Anki receives.
pub fn validate_deck_content(deck: &CanonicalDeck) -> ContentValidationReport {
    let mut report = ContentValidationReport::default();
    report.extend(validate_content_str(
        ContentKind::HtmlFragment,
        DeckPath::DeckDescription.to_string(),
        &deck.description,
    ));

    for (note_type_id, note_type) in &deck.note_types {
        report.extend(validate_content_str(
            ContentKind::Css,
            DeckPath::NoteTypeStyling {
                note_type_id: note_type_id.clone(),
            }
            .to_string(),
            &note_type.styling,
        ));
        for template in &note_type.card_templates {
            report.extend(validate_content_str(
                ContentKind::HtmlFragment,
                DeckPath::NoteTypeCardTemplateQuestionFormat {
                    note_type_id: note_type_id.clone(),
                    template_id: template.id.clone(),
                }
                .to_string(),
                &template.question_format,
            ));
            report.extend(validate_content_str(
                ContentKind::HtmlFragment,
                DeckPath::NoteTypeCardTemplateAnswerFormat {
                    note_type_id: note_type_id.clone(),
                    template_id: template.id.clone(),
                }
                .to_string(),
                &template.answer_format,
            ));
        }
    }

    report
}

fn validate_html_fragment(path: &str, source: &str) -> ContentValidationReport {
    let mut report = ContentValidationReport::default();
    let mut stack = Vec::<OpenTag>::new();
    let mut index = 0;
    while let Some(relative) = source[index..].find('<') {
        let tag_start = index + relative;
        let Some(next) = source[tag_start + 1..].chars().next() else {
            break;
        };

        if next == '!' {
            if source[tag_start..].starts_with("<!--") {
                if let Some(end) = source[tag_start + 4..].find("-->") {
                    index = tag_start + 4 + end + 3;
                } else {
                    report.push(ContentValidationError::new(
                        path,
                        ContentKind::HtmlFragment,
                        Some(line_number(source, tag_start)),
                        "unterminated HTML comment",
                    ));
                    break;
                }
            } else if let Some(end) = source[tag_start..].find('>') {
                index = tag_start + end + 1;
            } else {
                report.push(ContentValidationError::new(
                    path,
                    ContentKind::HtmlFragment,
                    Some(line_number(source, tag_start)),
                    "unterminated HTML declaration",
                ));
                break;
            }
            continue;
        }

        if next == '?' {
            if let Some(end) = source[tag_start..].find("?>") {
                index = tag_start + end + 2;
            } else if let Some(end) = source[tag_start..].find('>') {
                index = tag_start + end + 1;
            } else {
                report.push(ContentValidationError::new(
                    path,
                    ContentKind::HtmlFragment,
                    Some(line_number(source, tag_start)),
                    "unterminated HTML processing instruction",
                ));
                break;
            }
            continue;
        }

        if next != '/' && !next.is_ascii_alphabetic() {
            index = tag_start + 1;
            continue;
        }

        let Some(tag_end) = find_tag_end(source, tag_start + 1) else {
            report.push(ContentValidationError::new(
                path,
                ContentKind::HtmlFragment,
                Some(line_number(source, tag_start)),
                "unterminated HTML tag",
            ));
            break;
        };
        let raw = &source[tag_start + 1..tag_end];
        let trimmed = raw.trim();
        if let Some(rest) = trimmed.strip_prefix('/') {
            if let Some(name) = html_tag_name(rest) {
                let line = line_number(source, tag_start);
                match stack.pop() {
                    Some(open) if open.name == name => {}
                    Some(open) => {
                        report.push(ContentValidationError::new(
                            path,
                            ContentKind::HtmlFragment,
                            Some(line),
                            format!(
                                "mismatched closing tag </{name}>; expected </{}> for <{}> opened on line {}",
                                open.name, open.name, open.line
                            ),
                        ));
                        stack.push(open);
                    }
                    None => report.push(ContentValidationError::new(
                        path,
                        ContentKind::HtmlFragment,
                        Some(line),
                        format!("stray closing tag </{name}>"),
                    )),
                }
            }
        } else if let Some(name) = html_tag_name(trimmed)
            && !is_void_html_element(&name)
            && !html_tag_is_self_closing(trimmed)
        {
            stack.push(OpenTag {
                name,
                line: line_number(source, tag_start),
            });
        }
        index = tag_end + 1;
    }

    for open in stack {
        report.push(ContentValidationError::new(
            path,
            ContentKind::HtmlFragment,
            Some(open.line),
            format!("unclosed HTML tag <{}>", open.name),
        ));
    }
    report
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OpenTag {
    name: String,
    line: usize,
}

fn validate_css(path: &str, source: &str) -> ContentValidationReport {
    let mut report = ContentValidationReport::default();
    let stripped = match strip_css_comments_and_strings(source) {
        Ok(stripped) => stripped,
        Err(error) => {
            report.push(ContentValidationError::new(
                path,
                ContentKind::Css,
                None,
                error,
            ));
            return report;
        }
    };

    let mut stack = Vec::<(char, usize)>::new();
    for (line_index, line) in stripped.lines().enumerate() {
        let line_no = line_index + 1;
        for ch in line.chars() {
            match ch {
                '{' | '(' | '[' => stack.push((ch, line_no)),
                '}' | ')' | ']' => {
                    let expected = matching_opener(ch);
                    if stack.last().map(|(open, _)| *open) == Some(expected) {
                        stack.pop();
                    } else {
                        report.push(ContentValidationError::new(
                            path,
                            ContentKind::Css,
                            Some(line_no),
                            format!("unmatched {ch:?}"),
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    for (ch, line_no) in stack {
        report.push(ContentValidationError::new(
            path,
            ContentKind::Css,
            Some(line_no),
            format!("unmatched {ch:?}"),
        ));
    }
    report
}

fn strip_css_comments_and_strings(source: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut chars = source.char_indices().peekable();
    let mut quote = None::<char>;
    let mut in_comment = false;

    while let Some((_, ch)) = chars.next() {
        let next = chars.peek().map(|(_, ch)| *ch).unwrap_or('\0');
        if in_comment {
            if ch == '*' && next == '/' {
                in_comment = false;
                chars.next();
            }
            continue;
        }
        if let Some(active_quote) = quote {
            if ch == '\\' {
                chars.next();
                continue;
            }
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if ch == '/' && next == '*' {
            in_comment = true;
            chars.next();
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }
        out.push(ch);
    }

    if in_comment {
        return Err("unterminated CSS comment".to_owned());
    }
    if quote.is_some() {
        return Err("unterminated CSS string".to_owned());
    }
    Ok(out)
}

fn find_tag_end(source: &str, mut index: usize) -> Option<usize> {
    let mut quote = None::<char>;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
        } else if ch == '"' || ch == '\'' {
            quote = Some(ch);
        } else if ch == '>' {
            return Some(index);
        }
        index += ch.len_utf8();
    }
    None
}

fn html_tag_name(raw: &str) -> Option<String> {
    let mut name = String::new();
    for ch in raw.trim_start().chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | ':') {
            name.push(ch.to_ascii_lowercase());
        } else {
            break;
        }
    }
    (!name.is_empty()).then_some(name)
}

fn html_tag_is_self_closing(raw: &str) -> bool {
    raw.trim_end().ends_with('/')
}

fn is_void_html_element(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn matching_opener(closer: char) -> char {
    match closer {
        '}' => '{',
        ')' => '(',
        ']' => '[',
        _ => unreachable!("not a CSS closer"),
    }
}

fn line_number(source: &str, byte_index: usize) -> usize {
    source[..byte_index]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}
