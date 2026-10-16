use crate::runtime::AuthService;
use dioxus::prelude::*;
use music_core::domain::{SourceCommand, SourceSnapshot};

fn initials(name: &str) -> String {
    let letters: String = name
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .collect();
    if letters.is_empty() {
        "T".into()
    } else {
        letters.to_uppercase()
    }
}

#[component]
pub fn AccountView(snapshot: SourceSnapshot, on_back: EventHandler<()>) -> Element {
    let mut confirm_signout = use_signal(|| false);
    let mut local_error = use_signal(|| None::<String>);
    let display_name = snapshot.account_name.clone();
    let username = snapshot.username.clone();
    let photo = snapshot.profile_photo_data.clone();
    rsx! {
        main { class: "utility-screen", aria_label: "Account",
            div { class: "utility-content",
                header { class: "utility-header",
                    button { class: "utility-back", r#type: "button", aria_label: "Back to Settings", onclick: move |_| on_back.call(()),
                        svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", path { d: "m15 18-6-6 6-6" } }
                    }
                    h1 { "Account" }
                }
                p { class: "utility-intro", "Your connected Telegram profile." }
                section { class: "account-profile", aria_label: "Telegram profile",
                    div { class: "account-avatar",
                        if let Some(photo) = photo { img { src: photo, alt: "" } }
                        else { span { aria_hidden: "true", "{initials(&display_name)}" } }
                    }
                    div { class: "account-details",
                        strong { dir: "auto", "{display_name}" }
                        if let Some(username) = username { span { dir: "auto", "@{username}" } }
                        else { span { "No public username" } }
                    }
                }
                if let Some(error) = local_error.read().clone().or(snapshot.error.clone()) {
                    p { class: "account-error", role: "alert", "{error}" }
                }
                section { class: "account-actions", aria_label: "Account actions",
                    p { "Signing out removes this account's library, listening history, downloads, and Telegram data from this device." }
                    button { class: "account-signout", r#type: "button", onclick: move |_| confirm_signout.set(true), "Sign out" }
                }
            }
        }
        if *confirm_signout.read() {
            div { class: "storage-dialog-backdrop", role: "presentation",
                div { class: "storage-dialog", role: "alertdialog", aria_modal: "true", aria_labelledby: "account-signout-title", aria_describedby: "account-signout-description",
                    h2 { id: "account-signout-title", "Sign out of Telegram?" }
                    p { id: "account-signout-description", "Playback and downloads will stop. Your library, favorites, listening history, source choices, offline songs, and local Telegram data will be removed from this device. Your Telegram messages stay in Telegram." }
                    div { class: "storage-dialog-actions",
                        button { r#type: "button", onclick: move |_| confirm_signout.set(false), "Cancel" }
                        button { class: "danger", r#type: "button", onclick: move |_| {
                            match AuthService::global().and_then(|service| service.submit_source(SourceCommand::ChangeAccount)) {
                                Ok(()) => { confirm_signout.set(false); local_error.set(None); }
                                Err(message) => local_error.set(Some(message)),
                            }
                        }, "Sign out" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::initials;

    #[test]
    fn avatar_initials_fallback_handles_empty_and_mixed_names() {
        assert_eq!(initials(""), "T");
        assert_eq!(initials("Ada Lovelace"), "AL");
        assert_eq!(initials("علی رضایی"), "عر");
    }
}
