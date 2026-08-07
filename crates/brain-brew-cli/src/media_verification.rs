//! Release media verification policy shared by verify and export.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum MediaVerificationMode {
    #[default]
    Strict,
    ReferenceOnly,
}

impl MediaVerificationMode {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "strict" => Ok(Self::Strict),
            "reference-only" => Ok(Self::ReferenceOnly),
            other => Err(format!(
                "invalid media verification mode {other:?}; expected strict or reference-only"
            )),
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::ReferenceOnly => "reference_only",
        }
    }

    pub(crate) fn release_ready(self, declaration_count: usize) -> bool {
        declaration_count == 0 || self == Self::Strict
    }

    pub(crate) fn development_warning(self, declaration_count: usize) -> Option<String> {
        (self == Self::ReferenceOnly && declaration_count > 0).then(|| {
            "MEDIA REFERENCE-ONLY DEVELOPMENT MODE: declaration/reference consistency was checked, but media roots and asset bytes were not validated; this result is NOT RELEASE-READY"
                .to_owned()
        })
    }
}
