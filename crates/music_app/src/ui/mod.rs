//! Dioxus entry page. The login design will replace this empty surface later.

use dioxus::prelude::*;

const STYLESHEET: Asset = asset!("/assets/style.css");

/// Root UI component for the Android WebView.
#[allow(non_snake_case)]
pub fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: STYLESHEET }
        BlankPage {}
    }
}

/// Empty canvas reserved for the future login page.
#[allow(non_snake_case)]
fn BlankPage() -> Element {
    rsx! {
        main { class: "blank-page" }
    }
}
