pub(crate) fn general() -> String {
    format!(
        concat!(
            "Brain Brew {}\n",
            "Local-first deck federation and round-trip tooling for Anki-compatible decks.\n\n",
            "Usage:\n",
            "  brainbrew <command> [options]\n\n",
            "Commands:\n",
            "  fmt       Format deck, overlay, manifest, or lock YAML in place\n",
            "  validate  Validate a deck file or manifest target\n",
            "  compose   Compose a base deck plus overlays into resolved CanonicalDeck YAML\n",
            "  export    Export a resolved deck to an adapter format, currently CrowdAnki\n",
            "  import    Import CrowdAnki into CanonicalDeck YAML\n",
            "  lock      Update or verify locked federated package inputs\n",
            "  targets   List manifest targets\n",
            "  verify    Run manifest formatting, composition, validation, media, and golden checks\n",
            "  explain   Explain a manifest target and its overlay stack\n",
            "  diff      Compare decks semantically, or emit an overlay draft\n\n",
            "Examples:\n",
            "  brainbrew targets --manifest brainbrew.yaml\n",
            "  brainbrew validate --manifest brainbrew.yaml --target da-standard\n",
            "  brainbrew compose --manifest brainbrew.yaml --target da-standard --out build/da.yaml\n",
            "  brainbrew compose --manifest america/brainbrew.yaml --include ultimate-geography/brainbrew.yaml --target en-america\n",
            "  brainbrew lock update --package anki-geo.ultimate-geography --path ../ultimate-geography\n",
            "  brainbrew export crowdanki --manifest brainbrew.yaml --target de-extended --media-root media/\n",
            "  brainbrew verify --manifest brainbrew.yaml --all-targets\n\n",
            "Run `brainbrew <command> --help` for command-specific examples.\n",
        ),
        env!("CARGO_PKG_VERSION")
    )
}

pub(crate) fn command(name: &str) -> Option<&'static str> {
    match name {
        "fmt" => Some(
            "Usage:\n  brainbrew fmt <deck-or-overlay-or-manifest-or-lock.yaml>\n\nExamples:\n  brainbrew fmt deck.yaml\n  brainbrew fmt overlays/languages/da.yaml\n  brainbrew fmt brainbrew.yaml\n  brainbrew fmt brainbrew.lock\n",
        ),
        "validate" => Some(
            "Usage:\n  brainbrew validate <deck.yaml> [--overlay overlay.yaml ...] [--json]\n  brainbrew validate --manifest brainbrew.yaml --target <target> [--include package/brainbrew.yaml ...] [--package-root packages/] [--json]\n\nExamples:\n  brainbrew validate deck.yaml\n  brainbrew validate deck.yaml --overlay overlays/languages/da.yaml\n  brainbrew validate --manifest brainbrew.yaml --target da-standard\n  brainbrew validate --manifest america/brainbrew.yaml --include ultimate-geography/brainbrew.yaml --target en-america\n  brainbrew validate --manifest brainbrew.yaml --target de-extended --json\n",
        ),
        "compose" => Some(
            "Usage:\n  brainbrew compose <deck.yaml> [--overlay overlay.yaml ...] [--out resolved.yaml]\n  brainbrew compose [--manifest brainbrew.yaml] --target <target> [--include package/brainbrew.yaml ...] [--package-root packages/] [--out resolved.yaml]\n\nExamples:\n  brainbrew compose deck.yaml --overlay overlays/languages/da.yaml --out build/da.yaml\n  brainbrew compose --manifest brainbrew.yaml --target da-standard --out build/da.yaml\n  brainbrew compose --manifest america/brainbrew.yaml --include ultimate-geography/brainbrew.yaml --target en-america\n  brainbrew compose --manifest brainbrew.yaml --target da-standard\n",
        ),
        "export" => Some(
            "Usage:\n  brainbrew export crowdanki <deck.yaml> [--overlay overlay.yaml ...] --out build/deck-folder\n  brainbrew export crowdanki [--manifest brainbrew.yaml] --target <target> [--include package/brainbrew.yaml ...] [--package-root packages/] [--out build/deck-folder] [--media-root media/]\n\nExamples:\n  brainbrew export crowdanki deck.yaml --overlay overlays/languages/da.yaml --out build/da-crowdanki\n  brainbrew export crowdanki --manifest brainbrew.yaml --target da-standard\n  brainbrew export crowdanki --manifest america/brainbrew.yaml --include ultimate-geography/brainbrew.yaml --target en-america --out build/en-america\n  brainbrew export crowdanki --manifest brainbrew.yaml --target de-extended --media-root media/\n\nWhen --out is omitted for a manifest target, the output defaults to build/crowdanki/<target> unless exports.crowdanki.out is configured.\n",
        ),
        "import" => Some(
            "Usage:\n  brainbrew import crowdanki <deck-folder> --accept-suggested-ids --out deck.yaml\n\nExamples:\n  brainbrew import crowdanki build/de-extended --accept-suggested-ids --out deck.yaml\n",
        ),
        "lock" => Some(
            "Usage:\n  brainbrew lock update --package <package-id> (--path <dir> | --git <github-url> [--ref <ref>] [--rev <rev>] | --tarball <url>) [--package-manifest brainbrew.yaml] [--lock brainbrew.lock]\n  brainbrew lock verify [--lock brainbrew.lock]\n\nExamples:\n  brainbrew lock update --package anki-geo.ultimate-geography --path ../ultimate-geography\n  brainbrew lock update --package anki-geo.ultimate-geography --git https://github.com/anki-geo/ultimate-geography.git --ref main\n  brainbrew lock verify\n",
        ),
        "targets" => Some(
            "Usage:\n  brainbrew targets [--manifest brainbrew.yaml] [--include package/brainbrew.yaml ...] [--package-root packages/] [--json]\n\nExamples:\n  brainbrew targets --manifest brainbrew.yaml\n  brainbrew targets --manifest brainbrew.yaml --json\n  brainbrew targets --package-root ../packages\n",
        ),
        "verify" => Some(
            "Usage:\n  brainbrew verify [--manifest brainbrew.yaml] (--all-targets | --target <target>) [--include package/brainbrew.yaml ...] [--package-root packages/] [--media-root media/]\n\nExamples:\n  brainbrew verify --manifest brainbrew.yaml --all-targets\n  brainbrew verify --manifest brainbrew.yaml --target da-standard\n  brainbrew verify --manifest america/brainbrew.yaml --include ultimate-geography/brainbrew.yaml --target en-america\n  brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media/\n",
        ),
        "explain" => Some(
            "Usage:\n  brainbrew explain [--manifest brainbrew.yaml] --target <target> [--include package/brainbrew.yaml ...] [--package-root packages/] [--json]\n\nExamples:\n  brainbrew explain --manifest brainbrew.yaml --target da-standard\n  brainbrew explain --manifest america/brainbrew.yaml --include ultimate-geography/brainbrew.yaml --target en-america\n  brainbrew explain --manifest brainbrew.yaml --target de-extended --json\n",
        ),
        "diff" => Some(
            "Usage:\n  brainbrew diff <left.yaml> <right.yaml> [--json]\n  brainbrew diff <left.yaml> <right.yaml> --as-overlay --id <overlay-id> [--kind patch]\n\nExamples:\n  brainbrew diff deck.yaml edited.yaml\n  brainbrew diff deck.yaml edited.yaml --json\n  brainbrew diff deck.yaml edited.yaml --as-overlay --id overlay.patch.capitals --kind patch\n",
        ),
        _ => None,
    }
}

pub(crate) fn usage_error(command: &str, fallback: &str) -> String {
    self::command(command)
        .map(str::to_owned)
        .unwrap_or_else(|| fallback.to_owned())
}
