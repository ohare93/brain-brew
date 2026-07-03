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
            "  media     Verify and refresh declared media asset hashes\n",
            "  targets   List manifest targets\n",
            "  translations Report/apply translation coverage (aliases: translate, translation)\n",
            "  workbench Serve the local Deck Workbench browser UI and JSON API\n",
            "  verify    Run manifest formatting, composition, validation, media, and golden checks\n",
            "  explain   Explain a manifest target and its overlay stack\n",
            "  diff      Compare decks semantically, or emit an overlay draft\n\n",
            "Examples:\n",
            "  brainbrew targets --manifest brainbrew.yaml\n",
            "  brainbrew validate --manifest brainbrew.yaml --target da-standard\n",
            "  brainbrew compose --manifest brainbrew.yaml --target da-standard --out build/da.yaml\n",
            "  brainbrew compose --manifest america/brainbrew.yaml --include ultimate-geography/brainbrew.yaml --target en-america\n",
            "  brainbrew lock update --package anki-geo.ultimate-geography --path ../ultimate-geography\n",
            "  brainbrew media hash --manifest brainbrew.yaml --all-targets --media-root media/\n",
            "  brainbrew export crowdanki --manifest brainbrew.yaml --target de-extended --media-root media/\n",
            "  brainbrew translate --manifest brainbrew.yaml --target de-standard\n",
            "  brainbrew workbench serve --manifest brainbrew.yaml\n",
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
            "Usage:\n  brainbrew lock update --package <package-id> (--path <dir> | --git <github-url> [--ref <ref>] [--rev <rev>] | --tarball <url>) [--package-manifest brainbrew.yaml] [--lock brainbrew.lock]\n  brainbrew lock verify [--lock brainbrew.lock]\n\nExperimental: lock/package federation works, but brainbrew.lock and the brainbrew lock CLI surface may change incompatibly.\n\nExamples:\n  brainbrew lock update --package anki-geo.ultimate-geography --path ../ultimate-geography\n  brainbrew lock update --package anki-geo.ultimate-geography --git https://github.com/anki-geo/ultimate-geography.git --ref main\n  brainbrew lock verify\n",
        ),
        "media" => Some(
            "Usage:\n  brainbrew media hash [--manifest brainbrew.yaml] (--all-targets | --target <target>) [--include package/brainbrew.yaml ...] [--package-root packages/] --media-root media/\n\nExamples:\n  brainbrew media hash --manifest brainbrew.yaml --all-targets --media-root media/\n  brainbrew media hash --manifest brainbrew.yaml --target en-standard --media-root media/\n\nComputes SHA-256 hashes for declared media files and writes missing or stale values back to deck/overlay source YAML using the include-preserving canonical formatter.\n",
        ),
        "targets" => Some(
            "Usage:\n  brainbrew targets [--manifest brainbrew.yaml] [--include package/brainbrew.yaml ...] [--package-root packages/] [--json]\n\nExamples:\n  brainbrew targets --manifest brainbrew.yaml\n  brainbrew targets --manifest brainbrew.yaml --json\n  brainbrew targets --package-root ../packages\n",
        ),
        "translations" | "translate" | "translation" => Some(
            "Usage:\n  brainbrew translate [--manifest brainbrew.yaml] [--target <target> | --all-targets] [--language <code>] [--overlay <id-or-file>] [--note <note-id>] [--field <field-id>] [--source <text>] [--duplicates] [--status <status>] [--path-prefix <deck-path>] [--apply | --summary | --context] [--full] [--interactive | --no-interactive] [--json]\n\nInteractive examples:\n  brainbrew translate\n  brainbrew translate --manifest fixtures/ultimate-geography/brainbrew.yaml\n  brainbrew translate --interactive\n\nScriptable examples:\n  brainbrew translations --manifest brainbrew.yaml --target da-standard\n  brainbrew translations --manifest brainbrew.yaml --all-targets --language de\n  brainbrew translations --manifest brainbrew.yaml --target de-standard --note note.finland --field field.country\n  brainbrew translations --manifest brainbrew.yaml --target de-standard --context --source Georgia\n  brainbrew translations --manifest brainbrew.yaml --target da-standard --apply\n  brainbrew translations --manifest brainbrew.yaml --all-targets --summary --json\n  brainbrew translations --manifest brainbrew.yaml --target da-standard --json\n\nReport mode is the default and never edits files. In an interactive terminal, missing choices open arrow-key selectors for target, scope, and mode; language and overlay selectors appear only when they disambiguate the selected targets. The human report is translator-focused by default: structural/media/tag fallbacks are hidden and summarized. Use --full to include every scalar fallback. --context shows source and target strings in note/field/card context; combine it with --note, --field, --source, --duplicates, --status, and --language to navigate the translator context view. --summary exports compact per-language/per-overlay counts and de-duplicates identical reports across target variants; human summary output uses narrow aligned columns by default, and --summary --full adds overlay/file columns. --apply inserts source->source translation stubs for missing text fallbacks in non-interactive mode, while interactive apply lets you select rows with Space/Enter and choose one action for all selected rows (mark no-change, direct/contextual stub, ignore-path, skip) or decide per row. Use --path-prefix with paths from a diff to scope a changed-base review.\n",
        ),
        "workbench" => Some(
            "Usage:\n  brainbrew workbench serve [--manifest brainbrew.yaml] [--port <port>] [--no-open] [--dev-assets <dir>] [--media-root <dir>]\n\nExamples:\n  brainbrew workbench serve --manifest brainbrew.yaml\n  brainbrew workbench serve --manifest brainbrew.yaml --port 0 --no-open\n  brainbrew workbench serve --manifest brainbrew.yaml --dev-assets target/workbench-ui\n  brainbrew workbench serve --manifest brainbrew.yaml --media-root media/\n\nStarts a localhost Deck Workbench server bound to 127.0.0.1. Port 0 selects an available port. Release builds serve embedded UI assets from the brainbrew binary; --dev-assets serves a development asset directory such as the output of `devenv shell workbench-ui-watch`. Use --media-root to serve declared media assets from a directory outside the manifest root. The JSON API exposes health and workspace metadata for the browser UI.\n",
        ),
        "verify" => Some(
            "Usage:\n  brainbrew verify [--manifest brainbrew.yaml] (--all-targets | --target <target>) [--include package/brainbrew.yaml ...] [--package-root packages/] [--media-root media/] [--translation-coverage lenient|strict]\n\nExamples:\n  brainbrew verify --manifest brainbrew.yaml --all-targets\n  brainbrew verify --manifest brainbrew.yaml --target da-standard\n  brainbrew verify --manifest america/brainbrew.yaml --include ultimate-geography/brainbrew.yaml --target en-america\n  brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media/\n  brainbrew verify --manifest brainbrew.yaml --target de-release --translation-coverage strict\n",
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
