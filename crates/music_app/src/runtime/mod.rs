//! Service composition, commands, events, cancellation, and lifecycle ownership.
//!
//! The runtime owns long-running work so unmounting a page does not cancel
//! playback or downloads.
