use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

use crate::*;

impl CanonicalDeck {
    /// Plan and resolve every live note field through one deterministic dependency graph.
    ///
    /// The callback is the explicit adapter boundary for structured images. It is invoked at
    /// most once for each image field, including image fields shared by a message diamond.
    pub fn resolve_field_graph<F, E>(
        &self,
        mut lower_images: F,
    ) -> Result<ResolvedFieldGraph, FieldGraphReport>
    where
        F: FnMut(&StableId, &StableId, &[FieldImageReference]) -> Result<String, E>,
        E: fmt::Display,
    {
        FieldGraph::plan(self)?.resolve(&mut lower_images)
    }

    /// Resolve all live fields with Brain Brew's deterministic adapter-text image lowering.
    pub fn resolved_field_graph(&self) -> Result<ResolvedFieldGraph, FieldGraphReport> {
        self.resolve_field_graph(|note_id, field_id, images| {
            lower_images_from_deck(self, note_id, field_id, images)
        })
    }

    /// Resolve structured messages into final scalar values while preserving standalone images.
    pub fn resolve_structured_messages(&self) -> Result<Self, FieldGraphReport> {
        let graph = self.resolved_field_graph()?;
        let mut resolved = self.clone();
        for (note_id, note) in &mut resolved.notes {
            for (field_id, value) in &mut note.fields {
                if !matches!(value, FieldValue::Message(_)) {
                    continue;
                }
                let path = note_field_path(note_id, field_id);
                let rendered = graph
                    .get(&path)
                    .expect("a planned live message field has one resolved value");
                *value = FieldValue::Scalar(rendered.to_owned());
            }
        }
        Ok(resolved)
    }

    /// Resolve one field as semantic text using the same graph as validation and export.
    pub fn field_text(
        &self,
        note_id: &StableId,
        field_id: &StableId,
    ) -> Result<String, FieldGraphReport> {
        let path = note_field_path(note_id, field_id);
        let graph = self.resolved_field_graph()?;
        graph
            .get(&path)
            .map(str::to_owned)
            .ok_or_else(|| FieldGraphReport {
                errors: vec![FieldGraphError {
                    kind: FieldGraphErrorKind::MissingFieldValue,
                    note_id: note_id.clone(),
                    field_id: field_id.clone(),
                    consuming_path: path.clone(),
                    dependency: Some(path.clone()),
                    representation: None,
                    cycle: Vec::new(),
                    message: format!("note field {path:?} does not have a live value"),
                }],
            })
    }
}

#[derive(Clone, Debug)]
struct FieldNode<'a> {
    note_id: StableId,
    field_id: StableId,
    path: String,
    value: &'a FieldValue,
    dependencies: Vec<FieldDependency>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FieldDependency {
    path: String,
    consuming_path: String,
}

#[derive(Clone, Debug)]
pub(crate) struct FieldGraph<'a> {
    /// Canonical path order inherited from the deck's BTree maps.
    nodes: Vec<FieldNode<'a>>,
    /// Lookup only; traversal never depends on randomized HashMap iteration.
    indices: HashMap<String, usize>,
}

impl<'a> FieldGraph<'a> {
    pub(crate) fn plan(deck: &'a CanonicalDeck) -> Result<Self, FieldGraphReport> {
        let mut nodes = Vec::new();
        for (note_id, note) in &deck.notes {
            if deck
                .tombstones
                .blocking(&TombstoneAddress::Note {
                    note_id: note_id.clone(),
                })
                .is_some()
            {
                continue;
            }
            for (field_id, value) in &note.fields {
                if deck
                    .tombstones
                    .blocking(&TombstoneAddress::NoteField {
                        note_id: note_id.clone(),
                        field_id: field_id.clone(),
                    })
                    .is_some()
                {
                    continue;
                }
                let path = note_field_path(note_id, field_id);
                nodes.push(FieldNode {
                    note_id: note_id.clone(),
                    field_id: field_id.clone(),
                    path,
                    value,
                    dependencies: Vec::new(),
                });
            }
        }

        let indices = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.path.clone(), index))
            .collect::<HashMap<_, _>>();
        let mut errors = Vec::new();
        for node in &mut nodes {
            let FieldValue::Message(message) = node.value else {
                continue;
            };
            collect_message_dependencies(node, message, &mut errors);
        }

        for node in &nodes {
            for dependency in &node.dependencies {
                validate_dependency(deck, &indices, node, dependency, &mut errors);
            }
        }

        detect_cycles(&nodes, &indices, &mut errors);
        if errors.is_empty() {
            Ok(Self { nodes, indices })
        } else {
            Err(FieldGraphReport { errors })
        }
    }

    fn resolve<F, E>(self, lower_images: &mut F) -> Result<ResolvedFieldGraph, FieldGraphReport>
    where
        F: FnMut(&StableId, &StableId, &[FieldImageReference]) -> Result<String, E>,
        E: fmt::Display,
    {
        let mut memo = vec![None; self.nodes.len()];
        let mut order = Vec::with_capacity(self.nodes.len());
        for index in 0..self.nodes.len() {
            if memo[index].is_some() {
                continue;
            }
            if let Err(error) = resolve_node(
                index,
                &self.nodes,
                &self.indices,
                &mut memo,
                &mut order,
                lower_images,
            ) {
                return Err(FieldGraphReport {
                    errors: vec![*error],
                });
            }
        }
        {
            let values = self
                .nodes
                .iter()
                .zip(memo)
                .map(|(node, value)| {
                    (
                        node.path.clone(),
                        value.expect("each valid graph node was resolved"),
                    )
                })
                .collect();
            Ok(ResolvedFieldGraph { values, order })
        }
    }
}

fn collect_message_dependencies(
    node: &mut FieldNode<'_>,
    message: &StructuredMessage,
    errors: &mut Vec<FieldGraphError>,
) {
    if let Err(error) = message.validate_shape() {
        errors.push(graph_error(
            node,
            FieldGraphErrorKind::InvalidMessage,
            node.path.clone(),
            None,
            error.to_string(),
        ));
        return;
    }

    if let Some(format) = &message.format {
        match parse_message_format(format) {
            Ok(parts) => {
                let mut seen = HashSet::new();
                for variable in parts.iter().filter_map(|part| match part {
                    MessageFormatPart::Literal(_) => None,
                    MessageFormatPart::Variable(variable) => Some(variable),
                }) {
                    if seen.insert(variable) && !message.variables.contains_key(variable) {
                        errors.push(graph_error(
                            node,
                            FieldGraphErrorKind::InvalidMessage,
                            message_format_path(&node.note_id, &node.field_id),
                            None,
                            format!(
                                "structured message format references undefined variable {variable:?}"
                            ),
                        ));
                    }
                }
            }
            Err(message) => errors.push(graph_error(
                node,
                FieldGraphErrorKind::InvalidMessage,
                message_format_path(&node.note_id, &node.field_id),
                None,
                message,
            )),
        }
        for (variable, component) in &message.variables {
            collect_component_dependency(
                node,
                component,
                message_variable_path(&node.note_id, &node.field_id, variable),
                errors,
            );
        }
    } else {
        for (index, component) in message.components.iter().enumerate() {
            collect_component_dependency(
                node,
                component,
                message_component_path(&node.note_id, &node.field_id, index),
                errors,
            );
        }
    }
}

fn collect_component_dependency(
    node: &mut FieldNode<'_>,
    component: &MessageComponent,
    consuming_path: String,
    errors: &mut Vec<FieldGraphError>,
) {
    let MessageComponent::FieldRef(reference) = component else {
        return;
    };
    let Ok(DeckPath::NoteField { .. }) = reference.parse::<DeckPath>() else {
        errors.push(graph_error(
            node,
            FieldGraphErrorKind::InvalidReference,
            consuming_path,
            Some(reference.clone()),
            format!(
                "structured message field reference {reference:?} is not a canonical note field path"
            ),
        ));
        return;
    };
    node.dependencies.push(FieldDependency {
        path: reference.clone(),
        consuming_path,
    });
}

fn validate_dependency(
    deck: &CanonicalDeck,
    indices: &HashMap<String, usize>,
    consumer: &FieldNode<'_>,
    dependency: &FieldDependency,
    errors: &mut Vec<FieldGraphError>,
) {
    let DeckPath::NoteField { note_id, field_id } = dependency
        .path
        .parse::<DeckPath>()
        .expect("dependency paths were parsed while planning")
    else {
        unreachable!("dependency paths are note fields")
    };

    let note_address = TombstoneAddress::Note {
        note_id: note_id.clone(),
    };
    let field_address = TombstoneAddress::NoteField {
        note_id: note_id.clone(),
        field_id: field_id.clone(),
    };
    if deck.tombstones.blocking(&note_address).is_some()
        || deck.tombstones.blocking(&field_address).is_some()
    {
        errors.push(graph_error(
            consumer,
            FieldGraphErrorKind::TombstonedDependency,
            dependency.consuming_path.clone(),
            Some(dependency.path.clone()),
            format!(
                "structured message field reference {:?} resolves to a tombstoned field",
                dependency.path
            ),
        ));
        return;
    }

    let Some(note) = deck.notes.get(&note_id) else {
        errors.push(graph_error(
            consumer,
            FieldGraphErrorKind::MissingNote,
            dependency.consuming_path.clone(),
            Some(dependency.path.clone()),
            format!(
                "structured message field reference {:?} names missing note {note_id}",
                dependency.path
            ),
        ));
        return;
    };

    let definition_exists = deck
        .note_types
        .get(&note.note_type_id)
        .is_some_and(|note_type| note_type.fields.iter().any(|field| field.id == field_id));
    if !definition_exists {
        errors.push(graph_error(
            consumer,
            FieldGraphErrorKind::MissingFieldDefinition,
            dependency.consuming_path.clone(),
            Some(dependency.path.clone()),
            format!(
                "structured message field reference {:?} names field {field_id} without a definition on note type {}",
                dependency.path, note.note_type_id
            ),
        ));
        return;
    }

    if !indices.contains_key(&dependency.path) {
        errors.push(graph_error(
            consumer,
            FieldGraphErrorKind::MissingFieldValue,
            dependency.consuming_path.clone(),
            Some(dependency.path.clone()),
            format!(
                "structured message field reference {:?} names a field with no current value",
                dependency.path
            ),
        ));
    }
}

fn detect_cycles(
    nodes: &[FieldNode<'_>],
    indices: &HashMap<String, usize>,
    errors: &mut Vec<FieldGraphError>,
) {
    let mut state = vec![VisitState::Unvisited; nodes.len()];
    let mut stack = Vec::new();
    let mut positions = vec![None; nodes.len()];
    let mut cycles = HashSet::new();
    for index in 0..nodes.len() {
        detect_cycles_from(
            index,
            nodes,
            indices,
            &mut state,
            &mut stack,
            &mut positions,
            &mut cycles,
            errors,
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VisitState {
    Unvisited,
    Visiting,
    Complete,
}

#[allow(clippy::too_many_arguments)]
fn detect_cycles_from(
    index: usize,
    nodes: &[FieldNode<'_>],
    indices: &HashMap<String, usize>,
    state: &mut [VisitState],
    stack: &mut Vec<usize>,
    positions: &mut [Option<usize>],
    cycles: &mut HashSet<Vec<String>>,
    errors: &mut Vec<FieldGraphError>,
) {
    if state[index] != VisitState::Unvisited {
        return;
    }
    state[index] = VisitState::Visiting;
    positions[index] = Some(stack.len());
    stack.push(index);

    let node = &nodes[index];
    for dependency in &node.dependencies {
        let Some(&dependency_index) = indices.get(&dependency.path) else {
            continue;
        };
        match state[dependency_index] {
            VisitState::Visiting => {
                let start = positions[dependency_index]
                    .expect("visiting graph nodes have a stack position");
                let cycle = canonical_cycle(&stack[start..], nodes);
                if cycles.insert(cycle.clone()) {
                    let mut error = graph_error(
                        node,
                        FieldGraphErrorKind::Cycle,
                        dependency.consuming_path.clone(),
                        Some(dependency.path.clone()),
                        format!(
                            "structured message dependency cycle: {}",
                            cycle.join(" -> ")
                        ),
                    );
                    error.cycle = cycle;
                    errors.push(error);
                }
            }
            VisitState::Complete => {}
            VisitState::Unvisited => detect_cycles_from(
                dependency_index,
                nodes,
                indices,
                state,
                stack,
                positions,
                cycles,
                errors,
            ),
        }
    }

    stack.pop();
    positions[index] = None;
    state[index] = VisitState::Complete;
}

fn canonical_cycle(open_cycle: &[usize], nodes: &[FieldNode<'_>]) -> Vec<String> {
    let start = (0..open_cycle.len())
        .min_by_key(|index| &nodes[open_cycle[*index]].path)
        .unwrap_or(0);
    let mut cycle = (0..open_cycle.len())
        .map(|offset| {
            nodes[open_cycle[(start + offset) % open_cycle.len()]]
                .path
                .clone()
        })
        .collect::<Vec<_>>();
    if let Some(first) = cycle.first().cloned() {
        cycle.push(first);
    }
    cycle
}

fn resolve_node<F, E>(
    index: usize,
    nodes: &[FieldNode<'_>],
    indices: &HashMap<String, usize>,
    memo: &mut [Option<String>],
    order: &mut Vec<String>,
    lower_images: &mut F,
) -> Result<String, Box<FieldGraphError>>
where
    F: FnMut(&StableId, &StableId, &[FieldImageReference]) -> Result<String, E>,
    E: fmt::Display,
{
    if let Some(value) = &memo[index] {
        return Ok(value.clone());
    }
    let node = &nodes[index];
    for dependency in &node.dependencies {
        resolve_node(
            indices[&dependency.path],
            nodes,
            indices,
            memo,
            order,
            lower_images,
        )?;
    }
    let value = match node.value {
        FieldValue::Scalar(value) => value.clone(),
        FieldValue::Images(images) => {
            lower_images(&node.note_id, &node.field_id, images).map_err(|error| {
                let mut graph_error = graph_error(
                    node,
                    FieldGraphErrorKind::InvalidTargetRepresentation,
                    node.path.clone(),
                    Some(node.path.clone()),
                    format!(
                        "cannot lower structured images referenced at {}: {error}",
                        node.path
                    ),
                );
                graph_error.representation = Some(FieldValueKind::Images);
                Box::new(graph_error)
            })?
        }
        FieldValue::Message(message) => render_structured_message(message, indices, memo),
    };
    memo[index] = Some(value.clone());
    order.push(node.path.clone());
    Ok(value)
}

fn render_structured_message(
    message: &StructuredMessage,
    indices: &HashMap<String, usize>,
    memo: &[Option<String>],
) -> String {
    if let Some(format) = &message.format {
        let variables = message
            .variables
            .iter()
            .map(|(name, component)| {
                (
                    name.clone(),
                    render_message_component(component, indices, memo),
                )
            })
            .collect::<BTreeMap<_, _>>();
        return render_message_format(format, &variables)
            .expect("message format was validated while planning");
    }
    message
        .components
        .iter()
        .map(|component| render_message_component(component, indices, memo))
        .collect()
}

fn render_message_component(
    component: &MessageComponent,
    indices: &HashMap<String, usize>,
    memo: &[Option<String>],
) -> String {
    match component {
        MessageComponent::Literal(value) | MessageComponent::Text(value) => value.clone(),
        MessageComponent::FieldRef(reference) => memo[indices[reference]]
            .as_ref()
            .expect("dependencies resolve before their consuming message")
            .clone(),
    }
}

fn graph_error(
    node: &FieldNode<'_>,
    kind: FieldGraphErrorKind,
    consuming_path: String,
    dependency: Option<String>,
    message: String,
) -> FieldGraphError {
    FieldGraphError {
        kind,
        note_id: node.note_id.clone(),
        field_id: node.field_id.clone(),
        consuming_path,
        dependency,
        representation: None,
        cycle: Vec::new(),
        message,
    }
}

pub(crate) fn validate_message_graph(deck: &CanonicalDeck, errors: &mut Vec<ValidationError>) {
    if let Err(report) = FieldGraph::plan(deck) {
        errors.extend(
            report
                .errors
                .into_iter()
                .map(ValidationError::from_field_graph),
        );
    }
}

pub(crate) fn lower_images_from_deck(
    deck: &CanonicalDeck,
    _note_id: &StableId,
    _field_id: &StableId,
    images: &[FieldImageReference],
) -> Result<String, String> {
    let mut rendered = String::new();
    for image in images {
        let Some(media) = deck.media.get(&image.media_id) else {
            return Err(format!("unknown media id {:?}", image.media_id.as_str()));
        };
        let encoded_path = encode_media_path_for_url(&media.path);
        rendered.push_str("<img src=\"");
        rendered.push_str(&escape_html_attribute(&encoded_path));
        rendered.push_str("\" />");
    }
    Ok(rendered)
}

fn encode_media_path_for_url(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            encoded.push(char::from(*byte));
        } else {
            use std::fmt::Write as _;
            write!(&mut encoded, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    encoded
}

fn escape_html_attribute(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MessageFormatPart {
    Literal(String),
    Variable(String),
}

pub(crate) fn render_message_format(
    format: &str,
    variables: &BTreeMap<String, String>,
) -> Result<String, String> {
    let parts = parse_message_format(format)?;
    let mut rendered = String::new();
    for part in parts {
        match part {
            MessageFormatPart::Literal(value) => rendered.push_str(&value),
            MessageFormatPart::Variable(variable) => {
                let Some(value) = variables.get(&variable) else {
                    return Err(format!(
                        "structured message format references undefined variable {variable:?}"
                    ));
                };
                rendered.push_str(value);
            }
        }
    }
    Ok(rendered)
}

fn parse_message_format(format: &str) -> Result<Vec<MessageFormatPart>, String> {
    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut chars = format.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    literal.push('{');
                    continue;
                }
                if !literal.is_empty() {
                    parts.push(MessageFormatPart::Literal(std::mem::take(&mut literal)));
                }
                let mut variable = String::new();
                let mut closed = false;
                for variable_ch in chars.by_ref() {
                    if variable_ch == '}' {
                        closed = true;
                        break;
                    }
                    variable.push(variable_ch);
                }
                if !closed {
                    return Err(
                        "structured message format has an unclosed variable placeholder".to_owned(),
                    );
                }
                if variable.is_empty() {
                    return Err(
                        "structured message format contains an empty variable placeholder"
                            .to_owned(),
                    );
                }
                parts.push(MessageFormatPart::Variable(variable));
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    literal.push('}');
                } else {
                    return Err(
                        "structured message format contains an unmatched closing brace".to_owned(),
                    );
                }
            }
            other => literal.push(other),
        }
    }
    if !literal.is_empty() {
        parts.push(MessageFormatPart::Literal(literal));
    }
    Ok(parts)
}

pub(crate) fn message_component_path(
    note_id: &StableId,
    field_id: &StableId,
    index: usize,
) -> String {
    DeckPath::NoteFieldMessageComponent {
        note_id: note_id.clone(),
        field_id: field_id.clone(),
        index,
    }
    .to_string()
}

pub(crate) fn message_format_path(note_id: &StableId, field_id: &StableId) -> String {
    DeckPath::NoteFieldMessageFormat {
        note_id: note_id.clone(),
        field_id: field_id.clone(),
    }
    .to_string()
}

pub(crate) fn message_variable_path(
    note_id: &StableId,
    field_id: &StableId,
    variable: &str,
) -> String {
    DeckPath::NoteFieldMessageVariable {
        note_id: note_id.clone(),
        field_id: field_id.clone(),
        variable: variable.to_owned(),
    }
    .to_string()
}

pub(crate) fn note_field_path(note_id: &StableId, field_id: &StableId) -> String {
    DeckPath::NoteField {
        note_id: note_id.clone(),
        field_id: field_id.clone(),
    }
    .to_string()
}
