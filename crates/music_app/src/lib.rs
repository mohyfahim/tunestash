//! TuneStash application shell.
//!
//! UI, runtime, and platform adapters live here. Product behavior stays in
//! `music_core`, which this crate links.

pub mod adapters;
pub mod runtime;
pub mod ui;

/// Application package version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Name of the linked platform-independent core.
pub fn core_name() -> &'static str {
    music_core::crate_name()
}

#[cfg(test)]
mod tests {
    use super::{core_name, version};

    #[test]
    fn shell_links_the_core_crate() {
        assert_eq!(core_name(), "music_core");
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
