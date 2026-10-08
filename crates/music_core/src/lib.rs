//! Platform-independent domain, application logic, and ports.
//!
//! This crate owns product behavior. It must build and test on a host machine
//! without an Android SDK, a TDLib installation, or any UI framework.

#![forbid(unsafe_code)]

pub mod application;
pub mod domain;
pub mod ports;

/// Package name of the platform-independent core.
pub fn crate_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_name_matches_the_package() {
        assert_eq!(crate_name(), "music_core");
    }
}
