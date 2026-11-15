//! What counts as a version string worth showing: the registries, a
//! binary's `--version` and a Flatpak's metainfo are all held to it.

/// Letters, digits, `.`, `-` and `+`, at least one: the characters of a tag
/// or a semver. Anything else (markup, spaces) isn't shown as a version.
pub(crate) fn is_plain(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '+')
}

#[cfg(test)]
mod tests {
    use super::is_plain;

    #[test]
    fn tags_and_semvers_are_plain() {
        for version in ["1.3.1", "v8", "0.4.0-rc1", "1.0.0+build.5", "dev"] {
            assert!(is_plain(version), "{version}");
        }
    }

    #[test]
    fn markup_spaces_and_nothing_are_not() {
        for version in ["", "<b>1</b>", "1.0 beta", "1.0\n", "1/2", "v1_0"] {
            assert!(!is_plain(version), "{version:?}");
        }
    }
}
