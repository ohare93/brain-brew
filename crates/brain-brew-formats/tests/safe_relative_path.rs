use brain_brew_formats::safe_relative_path::{SafeRelativePath, SafeRelativePathError};

#[test]
fn accepts_only_portable_normal_relative_components() {
    for valid in [
        "deck.yaml",
        "overlays/languages/de.yaml",
        "percent/%2e%2e/literal.yaml",
        "unicode/․․/file.yaml",
        "media/旗 & map #1?.svg",
        "media/quote\"and'apostrophe.png",
    ] {
        let path = SafeRelativePath::new(valid)
            .unwrap_or_else(|error| panic!("{valid:?} should be safe: {error}"));
        assert_eq!(path.as_str(), valid);
        assert_eq!(path.to_path_buf().to_string_lossy(), valid);
    }
}

#[test]
fn rejects_empty_absolute_dot_parent_drive_unc_and_separator_ambiguity() {
    let cases = [
        ("", SafeRelativePathError::Empty),
        ("/etc/passwd", SafeRelativePathError::Absolute),
        ("//server/share", SafeRelativePathError::Absolute),
        (".", SafeRelativePathError::DotComponent),
        ("./deck.yaml", SafeRelativePathError::DotComponent),
        ("a/./deck.yaml", SafeRelativePathError::DotComponent),
        ("..", SafeRelativePathError::ParentComponent),
        ("../deck.yaml", SafeRelativePathError::ParentComponent),
        ("a/../deck.yaml", SafeRelativePathError::ParentComponent),
        ("C:/deck.yaml", SafeRelativePathError::WindowsDrivePrefix),
        ("c:deck.yaml", SafeRelativePathError::WindowsDrivePrefix),
        (r"C:\deck.yaml", SafeRelativePathError::WindowsDrivePrefix),
        (
            r"\\server\share\deck.yaml",
            SafeRelativePathError::WindowsUnc,
        ),
        (r"overlays\de.yaml", SafeRelativePathError::Backslash),
        ("a//deck.yaml", SafeRelativePathError::EmptyComponent),
        ("deck.yaml/", SafeRelativePathError::EmptyComponent),
        ("nul\0name", SafeRelativePathError::Nul),
        ("line\nfeed.png", SafeRelativePathError::Control),
        ("tab\tname.png", SafeRelativePathError::Control),
        ("http:asset.png", SafeRelativePathError::UrlSchemeDelimiter),
        ("bidi\u{202e}name.png", SafeRelativePathError::FormatControl),
        (
            "trailing-space /file.png",
            SafeRelativePathError::TrailingDotOrSpace,
        ),
        (
            "trailing-dot./file.png",
            SafeRelativePathError::TrailingDotOrSpace,
        ),
        ("CON.png", SafeRelativePathError::WindowsReservedName),
    ];

    for (raw, expected) in cases {
        assert_eq!(SafeRelativePath::new(raw).unwrap_err(), expected, "{raw:?}");
    }
}
