use dioxus::prelude::*;

#[component]
pub fn SettingsView(
    on_back: EventHandler<()>,
    on_sources: EventHandler<()>,
    on_storage: EventHandler<()>,
) -> Element {
    rsx! {
        main { class: "utility-screen", aria_label: "Settings",
            div { class: "utility-content",
                header { class: "utility-header",
                    button { class: "utility-back", r#type: "button", aria_label: "Back to Home",
                        onclick: move |_| on_back.call(()),
                        svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round", path { d: "m15 18-6-6 6-6" } }
                    }
                    h1 { "Settings" }
                }
                p { class: "utility-intro", "Choose where music comes from and how much space it uses." }
                nav { class: "settings-list", aria_label: "Settings sections",
                    button { class: "settings-link", r#type: "button", onclick: move |_| on_sources.call(()),
                        span { class: "settings-link-icon violet", aria_hidden: "true",
                            svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round", path { d: "M4 8.5 20 4l-4.5 16-4.2-6.1L4 8.5Zm7.3 5.4L20 4" } }
                        }
                        span { class: "settings-link-copy", strong { "Music Sources" } small { "Choose Telegram chats to include" } }
                        svg { class: "settings-chevron", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", path { d: "m9 5 7 7-7 7" } }
                    }
                    button { class: "settings-link", r#type: "button", onclick: move |_| on_storage.call(()),
                        span { class: "settings-link-icon lime", aria_hidden: "true",
                            svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round", rect { x: "3", y: "5", width: "18", height: "14", rx: "2" } path { d: "M3 10h18m-5 5h2" } }
                        }
                        span { class: "settings-link-copy", strong { "Storage management" } small { "Downloads, cache and streaming" } }
                        svg { class: "settings-chevron", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", path { d: "m9 5 7 7-7 7" } }
                    }
                }
            }
        }
    }
}
