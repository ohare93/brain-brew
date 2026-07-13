use std::fmt;

use crate::{StaleTranslation, TargetAdaptation, TranslationDictionary};

/// An atomic, validated mutation of a translation dictionary.
///
/// Faithful translations use non-empty source and target text. An intentional
/// blank target is a target adaptation, whose path and expected source make the
/// deletion explicit and reviewable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TranslationMutation {
    InsertDirect {
        source: String,
        target: String,
    },
    ReplaceDirect {
        source: String,
        expected_target: String,
        target: String,
    },
    SetContextual {
        occurrence_path: String,
        context: String,
        source: String,
        target: String,
    },
    MarkNoChange {
        source: String,
    },
    SetTargetAdaptation {
        path: String,
        expected_source: String,
        target: String,
        reason: Option<String>,
    },
}

/// A canonical repair chosen from translation coverage. Repairs remove only an
/// identified stale/invalid entry and are atomic as a batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TranslationDictionaryRepair {
    SetRequireComplete(bool),
    RemoveDirect { source: String },
    RemoveContextual { context: String, source: String },
    RemoveNoChange { source: String },
    RemoveTargetAdaptation { path: String },
    RemoveVariable { key: String, source: String },
    RemoveAdapterId { key: String, source: String },
}

/// A translator decision at one extracted source occurrence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TranslationDecision {
    Direct(String),
    Contextual { context: String, target: String },
    NoChange,
}

/// Translation-dictionary consequence of a compare-and-set source edit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceTranslationImpact {
    MarkStale {
        target: String,
        context: Option<String>,
    },
    MigrateKey {
        target: String,
        context: Option<String>,
    },
}

/// Deterministic missing-work batch for translation-report workflows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TranslationStubs {
    pub direct: std::collections::BTreeSet<String>,
    pub contextual: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
    pub no_change: std::collections::BTreeSet<String>,
    pub ignore_paths: std::collections::BTreeSet<String>,
}

/// Stable classification for rejected dictionary mutations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranslationMutationErrorKind {
    MissingKey,
    ExpectedValueMismatch,
    DuplicateKey,
    BlankFaithfulTranslation,
    EmptySource,
    InvalidContext,
    EmptyPath,
}

/// A deterministic mutation diagnostic that formats independently of YAML.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationMutationError {
    pub kind: TranslationMutationErrorKind,
    pub path: String,
    pub message: String,
}

impl TranslationMutationError {
    fn new(
        kind: TranslationMutationErrorKind,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            path: path.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for TranslationMutationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for TranslationMutationError {}

impl TranslationDictionary {
    /// Validate the format-independent invariants shared by every dictionary
    /// producer before any adapter renders or composes it.
    pub fn validate_mutation_invariants(&self) -> Result<(), TranslationMutationError> {
        for (source, target) in &self.direct {
            validate_faithful_translation(source, target, &direct_path(source))?;
        }
        for (context, replacements) in &self.contextual {
            if context.is_empty() {
                return Err(TranslationMutationError::new(
                    TranslationMutationErrorKind::EmptyPath,
                    "translations.contextual",
                    "translation context must not be empty",
                ));
            }
            for (source, target) in replacements {
                validate_faithful_translation(source, target, &contextual_path(context, source))?;
            }
        }
        for source in &self.no_change {
            validate_source(source, &format!("translations.no_change.{source}"))?;
        }
        for path in self.target_adaptations.keys() {
            if path.is_empty() {
                return Err(TranslationMutationError::new(
                    TranslationMutationErrorKind::EmptyPath,
                    "target_adaptations",
                    "target adaptation path must not be empty",
                ));
            }
        }
        for record in &self.stale_translations {
            validate_source(&record.old_source, "stale_translations")?;
            validate_faithful_translation(
                &record.new_source,
                &record.target,
                "stale_translations",
            )?;
            if record.context.as_deref() == Some("") {
                return Err(TranslationMutationError::new(
                    TranslationMutationErrorKind::EmptyPath,
                    "stale_translations",
                    "stale translation context must not be empty",
                ));
            }
        }
        Ok(())
    }

    /// Apply one mutation after validating all dictionary ownership invariants.
    pub fn apply_mutation(
        &mut self,
        mutation: TranslationMutation,
    ) -> Result<(), TranslationMutationError> {
        self.apply_mutations(std::slice::from_ref(&mutation))
    }

    /// Apply a batch as one transaction. A rejected later mutation leaves this
    /// dictionary unchanged, including stale-record ordering.
    pub fn apply_mutations(
        &mut self,
        mutations: &[TranslationMutation],
    ) -> Result<(), TranslationMutationError> {
        let mut next = self.clone();
        for mutation in mutations {
            next.apply_mutation_in_place(mutation)?;
        }
        next.canonicalize_stale_translations();
        *self = next;
        Ok(())
    }

    /// Apply coverage-derived cleanup without exposing raw map mutation to
    /// callers. Any invalid repair leaves this dictionary unchanged.
    pub fn apply_repairs(
        &mut self,
        repairs: &[TranslationDictionaryRepair],
    ) -> Result<(), TranslationMutationError> {
        let mut next = self.clone();
        for repair in repairs {
            next.apply_repair_in_place(repair)?;
        }
        next.validate_mutation_invariants()?;
        next.canonicalize_stale_translations();
        *self = next;
        Ok(())
    }

    /// Remove decisions superseded by an include-backed decision. The caller
    /// owns only storage routing; dictionary ownership remains in core.
    pub fn clear_superseded_translation_decisions(
        &mut self,
        occurrence_path: &str,
        source: &str,
        decision: &TranslationDecision,
    ) -> Result<(), TranslationMutationError> {
        let mut next = self.clone();
        validate_source(source, "translations")?;
        if occurrence_path.is_empty() {
            return Err(TranslationMutationError::new(
                TranslationMutationErrorKind::EmptyPath,
                "translations",
                "translation occurrence path must not be empty",
            ));
        }
        match decision {
            TranslationDecision::Direct(target) => {
                validate_faithful_translation(source, target, &direct_path(source))?;
                next.no_change.remove(source);
                next.remove_contextual_for_path(occurrence_path, source);
            }
            TranslationDecision::Contextual { context, target } => {
                let path = contextual_path(context, source);
                validate_faithful_translation(source, target, &path)?;
                validate_context(context, occurrence_path, &path)?;
                next.no_change.remove(source);
            }
            TranslationDecision::NoChange => {
                next.direct.remove(source);
                next.remove_contextual_for_path(occurrence_path, source);
                next.no_change.insert(source.to_owned());
            }
        }
        next.remove_stale_for_path_source(occurrence_path, source);
        next.canonicalize_stale_translations();
        *self = next;
        Ok(())
    }

    /// Install one translator decision while removing decisions that the new
    /// owner supersedes at the selected occurrence.
    pub fn set_translation_decision(
        &mut self,
        occurrence_path: &str,
        source: &str,
        decision: TranslationDecision,
    ) -> Result<(), TranslationMutationError> {
        let mut next = self.clone();
        let path = "translations";
        validate_source(source, path)?;
        if occurrence_path.is_empty() {
            return Err(TranslationMutationError::new(
                TranslationMutationErrorKind::EmptyPath,
                path,
                "translation occurrence path must not be empty",
            ));
        }
        match decision {
            TranslationDecision::Direct(target) => {
                validate_faithful_translation(source, &target, &direct_path(source))?;
                next.no_change.remove(source);
                next.remove_contextual_for_path(occurrence_path, source);
                next.direct.insert(source.to_owned(), target);
            }
            TranslationDecision::Contextual { context, target } => {
                let contextual_path = contextual_path(&context, source);
                validate_faithful_translation(source, &target, &contextual_path)?;
                validate_context(&context, occurrence_path, &contextual_path)?;
                next.no_change.remove(source);
                next.contextual
                    .entry(context)
                    .or_default()
                    .insert(source.to_owned(), target);
            }
            TranslationDecision::NoChange => {
                next.direct.remove(source);
                next.remove_contextual_for_path(occurrence_path, source);
                next.no_change.insert(source.to_owned());
            }
        }
        next.remove_stale_for_path_source(occurrence_path, source);
        next.canonicalize_stale_translations();
        *self = next;
        Ok(())
    }

    /// Record explicit review debt or a deliberate source-key migration.
    pub fn apply_source_translation_impact(
        &mut self,
        occurrence_path: &str,
        old_source: &str,
        new_source: &str,
        impact: SourceTranslationImpact,
    ) -> Result<(), TranslationMutationError> {
        validate_source(old_source, "translations")?;
        validate_source(new_source, "translations")?;
        if occurrence_path.is_empty() {
            return Err(TranslationMutationError::new(
                TranslationMutationErrorKind::EmptyPath,
                "translations",
                "source occurrence path must not be empty",
            ));
        }
        let (target, context, mark_stale) = match impact {
            SourceTranslationImpact::MarkStale { target, context } => (target, context, true),
            SourceTranslationImpact::MigrateKey { target, context } => (target, context, false),
        };
        validate_faithful_translation(new_source, &target, "translations")?;
        if let Some(context) = &context {
            validate_context(context, occurrence_path, "translations.contextual")?;
        }
        let mut next = self.clone();
        if context.is_some() {
            next.remove_contextual_for_path(occurrence_path, old_source);
        } else {
            next.direct.remove(old_source);
            next.no_change.remove(old_source);
            next.remove_contextual_everywhere(old_source);
        }
        next.remove_stale_for_path_source(occurrence_path, new_source);
        if mark_stale {
            next.stale_translations.retain(|record| {
                !(record.old_source == old_source
                    && record.new_source == new_source
                    && record.context == context)
            });
            next.stale_translations.push(StaleTranslation {
                old_source: old_source.to_owned(),
                new_source: new_source.to_owned(),
                target,
                context,
            });
        } else if let Some(context) = context {
            next.contextual
                .entry(context)
                .or_default()
                .insert(new_source.to_owned(), target);
        } else {
            next.direct.insert(new_source.to_owned(), target);
        }
        next.canonicalize_stale_translations();
        *self = next;
        Ok(())
    }

    /// Seed missing work without overwriting prior translator decisions.
    pub fn add_translation_stubs(
        &mut self,
        stubs: TranslationStubs,
    ) -> Result<(), TranslationMutationError> {
        let mut next = self.clone();
        for source in stubs.direct {
            validate_source(&source, "translations.direct")?;
            next.direct.entry(source.clone()).or_insert(source);
        }
        for (context, sources) in stubs.contextual {
            if context.is_empty() {
                return Err(TranslationMutationError::new(
                    TranslationMutationErrorKind::EmptyPath,
                    "translations.contextual",
                    "translation context must not be empty",
                ));
            }
            let replacements = next.contextual.entry(context).or_default();
            for source in sources {
                validate_source(&source, "translations.contextual")?;
                replacements.entry(source.clone()).or_insert(source);
            }
        }
        for source in stubs.no_change {
            validate_source(&source, "translations.no_change")?;
            next.no_change.insert(source);
        }
        next.ignore_paths.extend(stubs.ignore_paths);
        *self = next;
        Ok(())
    }

    /// Resolve a stale record into its normal direct/contextual owner. A
    /// current decision shadows the stale record and is never overwritten.
    pub fn resolve_stale_translation_decision(
        &mut self,
        old_source: &str,
        new_source: &str,
        context: Option<&str>,
        replacement: Option<&str>,
    ) -> Result<StaleTranslation, TranslationMutationError> {
        let mut next = self.clone();
        let position = next
            .stale_translations
            .iter()
            .position(|record| {
                record.old_source == old_source
                    && record.new_source == new_source
                    && record.context.as_deref() == context
            })
            .ok_or_else(|| {
                TranslationMutationError::new(
                    TranslationMutationErrorKind::MissingKey,
                    "stale_translations",
                    "matching stale translation was not found",
                )
            })?;
        let shadowed = stale_record_is_shadowed(&next, position);
        let mut record = next.stale_translations.remove(position);
        if !shadowed {
            if let Some(replacement) = replacement {
                validate_faithful_translation(
                    &record.new_source,
                    replacement,
                    "stale_translations",
                )?;
                record.target = replacement.to_owned();
            }
            if let Some(context) = record.context.clone() {
                next.contextual
                    .entry(context)
                    .or_default()
                    .insert(record.new_source.clone(), record.target.clone());
            } else {
                next.direct
                    .insert(record.new_source.clone(), record.target.clone());
            }
        }
        next.canonicalize_stale_translations();
        *self = next;
        Ok(record)
    }

    fn apply_repair_in_place(
        &mut self,
        repair: &TranslationDictionaryRepair,
    ) -> Result<(), TranslationMutationError> {
        let missing = |path: String| {
            TranslationMutationError::new(
                TranslationMutationErrorKind::MissingKey,
                path,
                "coverage repair target is missing",
            )
        };
        match repair {
            TranslationDictionaryRepair::SetRequireComplete(value) => {
                self.require_complete = *value;
            }
            TranslationDictionaryRepair::RemoveDirect { source } => {
                self.direct
                    .remove(source)
                    .ok_or_else(|| missing(direct_path(source)))?;
            }
            TranslationDictionaryRepair::RemoveContextual { context, source } => {
                let path = contextual_path(context, source);
                let replacements = self
                    .contextual
                    .get_mut(context)
                    .ok_or_else(|| missing(path.clone()))?;
                replacements.remove(source).ok_or_else(|| missing(path))?;
                if replacements.is_empty() {
                    self.contextual.remove(context);
                }
            }
            TranslationDictionaryRepair::RemoveNoChange { source } => {
                self.no_change
                    .remove(source)
                    .then_some(())
                    .ok_or_else(|| missing(format!("translations.no_change.{source}")))?;
            }
            TranslationDictionaryRepair::RemoveTargetAdaptation { path } => {
                self.target_adaptations
                    .remove(path)
                    .ok_or_else(|| missing(format!("target_adaptations.{path}")))?;
            }
            TranslationDictionaryRepair::RemoveVariable { key, source } => {
                let path = format!("translations.variables.{key}.{source}");
                let replacements = self
                    .variables
                    .get_mut(key)
                    .ok_or_else(|| missing(path.clone()))?;
                replacements.remove(source).ok_or_else(|| missing(path))?;
                if replacements.is_empty() {
                    self.variables.remove(key);
                }
            }
            TranslationDictionaryRepair::RemoveAdapterId { key, source } => {
                let path = format!("translations.adapter_ids.{key}.{source}");
                let replacements = self
                    .adapter_ids
                    .get_mut(key)
                    .ok_or_else(|| missing(path.clone()))?;
                replacements.remove(source).ok_or_else(|| missing(path))?;
                if replacements.is_empty() {
                    self.adapter_ids.remove(key);
                }
            }
        }
        Ok(())
    }

    fn apply_mutation_in_place(
        &mut self,
        mutation: &TranslationMutation,
    ) -> Result<(), TranslationMutationError> {
        match mutation {
            TranslationMutation::InsertDirect { source, target } => {
                let path = direct_path(source);
                validate_faithful_translation(source, target, &path)?;
                if self.direct.contains_key(source) {
                    return Err(TranslationMutationError::new(
                        TranslationMutationErrorKind::DuplicateKey,
                        path,
                        format!("direct translation source {source:?} already exists"),
                    ));
                }
                self.no_change.remove(source);
                self.direct.insert(source.clone(), target.clone());
            }
            TranslationMutation::ReplaceDirect {
                source,
                expected_target,
                target,
            } => {
                let path = direct_path(source);
                validate_faithful_translation(source, target, &path)?;
                let Some(current) = self.direct.get(source) else {
                    return Err(TranslationMutationError::new(
                        TranslationMutationErrorKind::MissingKey,
                        path,
                        format!("direct translation source {source:?} is missing"),
                    ));
                };
                if current != expected_target {
                    return Err(TranslationMutationError::new(
                        TranslationMutationErrorKind::ExpectedValueMismatch,
                        path,
                        format!(
                            "expected direct translation target {expected_target:?}, found {current:?}"
                        ),
                    ));
                }
                self.direct.insert(source.clone(), target.clone());
            }
            TranslationMutation::SetContextual {
                occurrence_path,
                context,
                source,
                target,
            } => {
                let path = contextual_path(context, source);
                validate_faithful_translation(source, target, &path)?;
                validate_context(context, occurrence_path, &path)?;
                self.no_change.remove(source);
                self.contextual
                    .entry(context.clone())
                    .or_default()
                    .insert(source.clone(), target.clone());
                self.remove_stale_for_path_source(occurrence_path, source);
            }
            TranslationMutation::MarkNoChange { source } => {
                let path = format!("translations.no_change.{source}");
                validate_source(source, &path)?;
                self.direct.remove(source);
                self.remove_contextual_everywhere(source);
                self.no_change.insert(source.clone());
            }
            TranslationMutation::SetTargetAdaptation {
                path,
                expected_source,
                target,
                reason,
            } => {
                if path.is_empty() {
                    return Err(TranslationMutationError::new(
                        TranslationMutationErrorKind::EmptyPath,
                        "target_adaptations",
                        "target adaptation path must not be empty",
                    ));
                }
                self.target_adaptations.insert(
                    path.clone(),
                    TargetAdaptation {
                        expected_source: expected_source.clone(),
                        target: target.clone(),
                        reason: reason.clone(),
                    },
                );
            }
        }
        Ok(())
    }

    fn canonicalize_stale_translations(&mut self) {
        self.stale_translations.sort_by(|left, right| {
            left.context
                .cmp(&right.context)
                .then_with(|| left.new_source.cmp(&right.new_source))
                .then_with(|| left.old_source.cmp(&right.old_source))
                .then_with(|| left.target.cmp(&right.target))
        });
    }

    fn remove_contextual_for_path(&mut self, path: &str, source: &str) {
        let contexts = self
            .contextual
            .iter()
            .filter(|(context, replacements)| {
                replacements.contains_key(source)
                    && (**context == path
                        || path
                            .strip_prefix(*context)
                            .is_some_and(|suffix| suffix.starts_with('.')))
            })
            .map(|(context, _)| context.clone())
            .collect::<Vec<_>>();
        for context in contexts {
            let replacements = self
                .contextual
                .get_mut(&context)
                .expect("context collected from this map remains present");
            replacements.remove(source);
            if replacements.is_empty() {
                self.contextual.remove(&context);
            }
        }
    }

    fn remove_contextual_everywhere(&mut self, source: &str) {
        let contexts = self
            .contextual
            .iter()
            .filter(|(_, replacements)| replacements.contains_key(source))
            .map(|(context, _)| context.clone())
            .collect::<Vec<_>>();
        for context in contexts {
            let replacements = self
                .contextual
                .get_mut(&context)
                .expect("context collected from this map remains present");
            replacements.remove(source);
            if replacements.is_empty() {
                self.contextual.remove(&context);
            }
        }
    }

    fn remove_stale_for_path_source(&mut self, path: &str, source: &str) {
        self.stale_translations.retain(|record| {
            !(record.new_source == source
                && record.context.as_deref().is_none_or(|context| {
                    context == path
                        || path
                            .strip_prefix(context)
                            .is_some_and(|suffix| suffix.starts_with('.'))
                }))
        });
    }
}

fn validate_faithful_translation(
    source: &str,
    target: &str,
    path: &str,
) -> Result<(), TranslationMutationError> {
    validate_source(source, path)?;
    if target.is_empty() {
        return Err(TranslationMutationError::new(
            TranslationMutationErrorKind::BlankFaithfulTranslation,
            path,
            "faithful translation target must not be blank; use no_change for reviewed unchanged text or a path-scoped target adaptation for an intentional deletion",
        ));
    }
    Ok(())
}

fn validate_source(source: &str, path: &str) -> Result<(), TranslationMutationError> {
    if source.is_empty() {
        return Err(TranslationMutationError::new(
            TranslationMutationErrorKind::EmptySource,
            path,
            "translation source must not be empty",
        ));
    }
    Ok(())
}

fn validate_context(
    context: &str,
    occurrence_path: &str,
    path: &str,
) -> Result<(), TranslationMutationError> {
    if context.is_empty() || occurrence_path.is_empty() {
        return Err(TranslationMutationError::new(
            TranslationMutationErrorKind::EmptyPath,
            path,
            "translation context and occurrence path must not be empty",
        ));
    }
    if context != occurrence_path
        && !occurrence_path
            .strip_prefix(context)
            .is_some_and(|suffix| suffix.starts_with('.'))
    {
        return Err(TranslationMutationError::new(
            TranslationMutationErrorKind::InvalidContext,
            path,
            format!("context {context:?} is not an ancestor of occurrence {occurrence_path:?}"),
        ));
    }
    Ok(())
}

fn stale_record_is_shadowed(translations: &TranslationDictionary, index: usize) -> bool {
    let record = &translations.stale_translations[index];
    translations.direct.contains_key(&record.new_source)
        || translations.no_change.contains(&record.new_source)
        || translations
            .contextual
            .iter()
            .any(|(context, replacements)| {
                replacements.contains_key(&record.new_source)
                    && record.context.as_deref().is_none_or(|stale_context| {
                        context == stale_context
                            || context
                                .strip_prefix(stale_context)
                                .is_some_and(|suffix| suffix.starts_with('.'))
                            || stale_context
                                .strip_prefix(context)
                                .is_some_and(|suffix| suffix.starts_with('.'))
                    })
            })
}

fn direct_path(source: &str) -> String {
    format!("translations.direct.{source}")
}

fn contextual_path(context: &str, source: &str) -> String {
    format!("translations.contextual.{context}.{source}")
}
