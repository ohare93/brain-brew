use brain_brew_core::{
    CanonicalDeck, FieldGraphReport, Overlay, TranslationCoverageCategory,
    TranslationDictionaryRepair,
};

pub(crate) fn compose_lenient_translation_overlay(
    current: &CanonicalDeck,
    overlay: &Overlay,
) -> Result<CanonicalDeck, String> {
    let sanitized = sanitize_lenient_translation_overlay(current, overlay)
        .map_err(|error| format!("failed to resolve translation source fields: {error}"))?;
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
) -> Result<Overlay, FieldGraphReport> {
    let mut sanitized = overlay.clone();
    if let Some(translations) = &mut sanitized.translations {
        let report = current.translation_coverage(overlay)?;
        let mut repairs = vec![TranslationDictionaryRepair::SetRequireComplete(false)];
        for entry in report.entries {
            match entry.category {
                TranslationCoverageCategory::StaleDirectKey => {
                    repairs.push(TranslationDictionaryRepair::RemoveDirect {
                        source: entry.source,
                    });
                }
                TranslationCoverageCategory::StaleContextualKey => {
                    if let Some(context) = entry.context {
                        repairs.push(TranslationDictionaryRepair::RemoveContextual {
                            context,
                            source: entry.source,
                        });
                    }
                }
                TranslationCoverageCategory::StaleNoChangeKey => {
                    repairs.push(TranslationDictionaryRepair::RemoveNoChange {
                        source: entry.source,
                    });
                }
                TranslationCoverageCategory::StaleTargetAdaptation
                | TranslationCoverageCategory::InvalidTargetAdaptation => {
                    repairs.push(TranslationDictionaryRepair::RemoveTargetAdaptation {
                        path: entry.context.unwrap_or(entry.path),
                    });
                }
                TranslationCoverageCategory::StaleVariableKey => {
                    if let Some(key) = entry.context {
                        repairs.push(TranslationDictionaryRepair::RemoveVariable {
                            key,
                            source: entry.source,
                        });
                    }
                }
                TranslationCoverageCategory::StaleAdapterIdKey => {
                    if let Some(key) = entry.context {
                        repairs.push(TranslationDictionaryRepair::RemoveAdapterId {
                            key,
                            source: entry.source,
                        });
                    }
                }
                _ => {}
            }
        }
        translations
            .apply_repairs(&repairs)
            .expect("coverage report repairs refer to entries in this dictionary");
    }
    Ok(sanitized)
}
