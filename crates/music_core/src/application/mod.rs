//! Import, search, favorites, queue policy, and sync orchestration.
//!
//! Use cases call ports and work with domain types. They do not talk to SQLite,
//! TDLib, or the mobile media stack directly.

/// A conservative search key. The imported metadata keeps its original spelling.
pub fn normalize_search(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            '\u{0643}' => '\u{06a9}',              // Arabic kaf → Persian kaf
            '\u{064a}' | '\u{0649}' => '\u{06cc}', // Arabic yeh/alef maqsura → Persian yeh
            other => other,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::normalize_search;

    #[test]
    fn normalizes_kaf_yeh_case_and_whitespace() {
        assert_eq!(normalize_search("  كيان\tMUSIC  "), "کیان music");
        assert_eq!(normalize_search("كيان"), normalize_search("کیان"));
        assert_eq!(normalize_search("على"), "علی");
    }
}
