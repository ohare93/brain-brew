use brain_brew_core::{CanonicalDeck, Overlay, TranslationCoverageCategory};

pub(crate) fn compose_lenient_translation_overlay(
    current: &CanonicalDeck,
    overlay: &Overlay,
) -> Result<CanonicalDeck, String> {
    let sanitized = sanitize_lenient_translation_overlay(current, overlay)?;
    current.compose(&[sanitized]).map_err(|error| {
        format!(
            "failed to compose translation overlay {}: {error}",
            overlay.id
        )
    })
}

pub(crate) fn sanitize_lenient_translation_overlay(
    current: &CanonicalDeck,
    overlay: &Overlay,
) -> Result<Overlay, String> {
    let mut sanitized = overlay.clone();
    if let Some(translations) = &mut sanitized.translations {
        translations.require_complete = false;
        let report = current
            .translation_coverage(overlay)
            .map_err(|error| format!("failed to resolve translation source fields: {error}"))?;
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
                TranslationCoverageCategory::StaleNoChangeKey => {
                    translations.no_change.remove(&entry.source);
                }
                TranslationCoverageCategory::StaleTargetAdaptation
                | TranslationCoverageCategory::InvalidTargetAdaptation => {
                    let path = entry.context.as_deref().unwrap_or(&entry.path);
                    translations.target_adaptations.remove(path);
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
                        && let Some(replacements) = translations.adapter_ids.get_mut(adapter_key)
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
    Ok(sanitized)
}
