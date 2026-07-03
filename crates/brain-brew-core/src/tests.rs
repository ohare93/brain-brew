use super::{CRATE_NAME, glob_matches};

#[test]
fn exposes_core_crate_name() {
    assert_eq!(CRATE_NAME, "brain-brew-core");
}

#[test]
fn glob_matches_table_driven_cases() {
    let cases = [
        ("literal-only match", "abc", "abc", true),
        ("literal-only mismatch", "abc", "ab", false),
        (
            "star at start matches prefix",
            "*world",
            "hello world",
            true,
        ),
        ("star at start can be empty", "*world", "world", true),
        ("star in middle matches run", "he*ld", "hello world", true),
        ("star in middle can be empty", "he*llo", "hello", true),
        ("star at end matches suffix", "hello*", "hello world", true),
        ("star at end can be empty", "hello*", "hello", true),
        (
            "multiple stars match ordered literals",
            "a*b*c",
            "axbyc",
            true,
        ),
        ("multiple stars can be empty", "a**b", "ab", true),
        (
            "multiple stars do not reorder literals",
            "a*b*c",
            "acb",
            false,
        ),
        (
            "multiple stars require repeated literals",
            "*a*a",
            "a",
            false,
        ),
        ("empty pattern matches empty input", "", "", true),
        ("empty pattern rejects non-empty input", "", "a", false),
        ("star matches empty input", "*", "", true),
        ("literal rejects empty input", "a", "", false),
        ("empty input with trailing star", "a*", "", false),
    ];

    for (case, pattern, value, expected) in cases {
        assert_eq!(glob_matches(pattern, value), expected, "{case}");
    }
}
