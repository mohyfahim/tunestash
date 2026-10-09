use crate::runtime::AuthService;
use dioxus::prelude::*;
use music_core::domain::{DiscoveryStage, SourceChat, SourceCommand, SourceKind, SourceSnapshot};

const NOTE: Asset = asset!("/assets/music-note.svg");

fn dispatch(command: SourceCommand, mut error: Signal<Option<String>>) {
    error.set(None);
    if let Err(message) = AuthService::global().and_then(|service| service.submit_source(command)) {
        error.set(Some(message));
    }
}

fn failed_title(snapshot: &SourceSnapshot, chat_id: i64) -> &str {
    snapshot
        .chats
        .iter()
        .find(|chat| chat.chat_id == chat_id)
        .map(|chat| chat.title.as_str())
        .unwrap_or("Unknown chat")
}

#[component]
pub fn SourceView(snapshot: SourceSnapshot, onboarding: bool) -> Element {
    let mut query = use_signal(String::new);
    let mut expanded_defaults = use_signal(|| false);
    let mut visible_chat_count = use_signal(|| 40usize);
    let mut confirm_change = use_signal(|| false);
    let error = use_signal(|| None::<String>);
    let loading = matches!(
        snapshot.stage,
        DiscoveryStage::Waiting | DiscoveryStage::LoadingChats | DiscoveryStage::CheckingMusic
    ) && !snapshot.show_partial;
    let failed_loading = snapshot.stage == DiscoveryStage::Failed && !snapshot.show_partial;
    let all_defaults: Vec<_> = snapshot.default_chats().cloned().collect();
    let default_count = all_defaults.len();
    let defaults: Vec<_> = all_defaults
        .into_iter()
        .take(if *expanded_defaults.read() {
            usize::MAX
        } else {
            5
        })
        .collect();
    let selected: Vec<_> = snapshot.selected_chats().cloned().collect();
    let term = query.read().trim().to_lowercase();
    let all_available: Vec<_> = snapshot
        .available_chats()
        .filter(|chat| {
            term.is_empty()
                || chat.title.to_lowercase().contains(&term)
                || chat.subtitle.to_lowercase().contains(&term)
        })
        .cloned()
        .collect();
    let available_count = all_available.len();
    let available: Vec<_> = all_available
        .into_iter()
        .take(*visible_chat_count.read())
        .collect();
    let progress = match snapshot.stage {
        DiscoveryStage::Waiting => "Preparing your Telegram chats".to_string(),
        DiscoveryStage::LoadingChats => "Loading Telegram chats".into(),
        DiscoveryStage::CheckingMusic => format!(
            "Checking music files · {} of {} chats checked",
            snapshot.checked_chats, snapshot.total_chats
        ),
        DiscoveryStage::Complete if snapshot.failed_checks > 0 => {
            "Discovery finished with gaps".into()
        }
        DiscoveryStage::Complete => "Your chats are ready".into(),
        DiscoveryStage::Failed => "Chat discovery paused".into(),
    };

    rsx! {
        if loading || failed_loading || snapshot.signing_out {
            main { class: "source-loading-screen",
                div { class: "source-loading-content",
                    div { class: "source-loading-mark", svg { view_box: "0 0 64 64", fill: "none",
                        circle { cx: "32", cy: "32", r: "29", stroke: "currentColor", stroke_width: "1.5" }
                        path { d: "M22 33h5m5-10v20m5-15v10m5-15v20", stroke: "currentColor", stroke_width: "3", stroke_linecap: "round" }
                    } }
                    h1 { if snapshot.signing_out { "Changing account" } else { "Finding your music sources" } }
                    p { class: "source-loading-description", if snapshot.signing_out { "Ending this Telegram session…" } else { "Looking through your chats for playable music files." } }
                    if !snapshot.signing_out {
                        div { class: "source-loading-line", role: "progressbar", aria_label: "Finding Telegram chats" }
                        p { class: "source-progress", role: "status", "{progress}" }
                    }
                    if let Some(message) = snapshot.error.as_ref() {
                        p { class: "source-message", role: "alert", "{message}" }
                    }
                    if !snapshot.failed_chats.is_empty() {
                        details { class: "failed-chat-details",
                            summary { "Show failed chat checks ({snapshot.failed_chats.len()})" }
                            ul {
                                for failure in &snapshot.failed_chats {
                                    li {
                                        strong { dir: "auto", "{failed_title(&snapshot, failure.chat_id)}" }
                                        span { "Chat ID {failure.chat_id} · {failure.operation} · Telegram {failure.code}: {failure.reason}" }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(message) = error.read().as_ref() {
                        p { class: "source-message", role: "alert", "{message}" }
                    }
                    if onboarding && snapshot.account_id.is_some() && !snapshot.signing_out {
                        button { class: "source-loading-change", r#type: "button", onclick: move |_| confirm_change.set(true), "Change Telegram account" }
                    }
                    if failed_loading {
                        div { class: "source-loading-actions",
                            button { class: "source-primary-action", r#type: "button", onclick: move |_| dispatch(SourceCommand::RetryDiscovery, error), "Retry search" }
                            button { class: "source-secondary-action", r#type: "button", onclick: move |_| dispatch(SourceCommand::ContinuePartial, error), "Continue with chats found" }
                        }
                    }
                }
            }
        } else {
            main { class: "sources-screen", id: "music-sources",
                div { class: "sources-layout",
                    header { class: "sources-header",
                        div { class: "sources-brand", img { class: "sources-brand-mark", src: NOTE, alt: "" } span { "Tune" span { "Stash" } } }
                        div { class: "sources-title-row",
                            h1 { "Music Sources" }
                            button { class: "source-resync", r#type: "button",
                                disabled: matches!(snapshot.stage, DiscoveryStage::Waiting | DiscoveryStage::LoadingChats | DiscoveryStage::CheckingMusic) || snapshot.signing_out,
                                onclick: move |_| dispatch(SourceCommand::Resync, error),
                                "Resync"
                            }
                        }
                        p { "Choose where TuneStash will find music. Resync when you want to check Telegram again." }
                    }
                    if onboarding {
                        div { class: "connected-account",
                            div { class: "connected-icon", aria_hidden: "true", svg { view_box: "0 0 24 24", path { d: "m21 3-3.1 17.3c-.1.7-.6.9-1.2.5l-5.1-3.8-2.5 2.4c-.3.3-.6.5-1 .5l.4-5.3L18.3 6c.3-.3-.1-.5-.4-.3L5.1 13.4l-4.4-1.5c-.6-.2-.6-.8.1-1.1L19.8 2.7c.8-.3 1.4 0 1.2.3Z" } } }
                            div { class: "connected-info", strong { "Connected to Telegram" } span { dir: "auto", "{snapshot.account_name}" } }
                            button { class: "source-link", r#type: "button", onclick: move |_| confirm_change.set(true), "Change" }
                        }
                    }
                    if snapshot.stage == DiscoveryStage::Failed || snapshot.stage == DiscoveryStage::CheckingMusic || snapshot.failed_checks > 0 {
                        div { class: "discovery-banner", role: "status",
                            span { "{progress}" }
                            if snapshot.stage == DiscoveryStage::Failed || (snapshot.stage == DiscoveryStage::Complete && snapshot.failed_checks > 0) {
                                button { r#type: "button", onclick: move |_| dispatch(SourceCommand::RetryDiscovery, error), "Retry" }
                            }
                        }
                    }
                    if let Some(message) = snapshot.error.as_ref() {
                        p { class: "source-message", role: "alert", "{message}" }
                    }
                    if !snapshot.failed_chats.is_empty() {
                        details { class: "failed-chat-details",
                            summary { "Show failed chat checks ({snapshot.failed_chats.len()})" }
                            ul {
                                for failure in &snapshot.failed_chats {
                                    li {
                                        strong { dir: "auto", "{failed_title(&snapshot, failure.chat_id)}" }
                                        span { "Chat ID {failure.chat_id} · {failure.operation} · Telegram {failure.code}: {failure.reason}" }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(message) = error.read().as_ref() {
                        p { class: "source-message", role: "alert", "{message}" }
                    }
                    section { class: "sources-section", aria_label: "Default sources",
                        div { class: "section-heading", h2 { "Default Sources" } p { "Your personal archive, channels, and music bots." } }
                        div { class: "default-list",
                            for chat in defaults { SourceRow { key: "default-{chat.chat_id}", chat, compact: false } }
                        }
                        if default_count == 0 { p { class: "source-empty", "No default sources with music found yet." } }
                        if default_count > 5 {
                            button { class: "source-expand", r#type: "button", onclick: move |_| expanded_defaults.toggle(),
                                if *expanded_defaults.read() { "Show fewer default sources" } else { "Show all {default_count} default sources" }
                            }
                        }
                    }
                    section { class: "sources-section", aria_label: "Selected chats",
                        div { class: "section-heading heading-with-action",
                            div { h2 { "Selected Chats" } p { "Other chats you've chosen for your library." } }
                            a { href: "#available-chats", class: "source-link", "+ Add source" }
                        }
                        if selected.is_empty() { p { class: "source-empty", "Music chats you add below will appear here." } }
                        else { div { class: "chat-list", for chat in selected { SourceRow { key: "selected-{chat.chat_id}", chat, compact: true } } } }
                    }
                    section { class: "sources-section", id: "available-chats", aria_label: "All chats",
                        div { class: "section-heading", h2 { "All Chats" } p { "Choose more conversations to include." } }
                        label { class: "chat-search",
                            svg { view_box: "0 0 24 24", fill: "none", circle { cx: "10.8", cy: "10.8", r: "6.8" } path { d: "m16 16 5 5" } }
                            input { r#type: "search", placeholder: "Search chats, channels, or bots", aria_label: "Search chats, channels, or bots", value: "{query}", oninput: move |event| query.set(event.value()) }
                        }
                        if available.is_empty() {
                            p { class: "source-empty", if term.is_empty() { if snapshot.stage == DiscoveryStage::CheckingMusic { "More music chats may appear as discovery continues." } else { "No unselected chats with music were found." } } else { "No music chats match your search." } }
                        } else {
                            div { class: "chat-list", for chat in available { SourceRow { key: "available-{chat.chat_id}", chat, compact: true } } }
                            if available_count > *visible_chat_count.read() {
                                button { class: "source-expand", r#type: "button", onclick: move |_| visible_chat_count.with_mut(|count| *count += 40), "Show more chats" }
                            }
                        }
                    }
                }
            }
        }
        if *confirm_change.read() {
            div { class: "source-dialog-backdrop", role: "presentation",
                div { class: "source-dialog", role: "dialog", aria_modal: "true", aria_labelledby: "change-account-title",
                    h2 { id: "change-account-title", "Change Telegram account?" }
                    p { "Signing out removes this account's saved source choices and local Telegram data from this device. Your Telegram messages stay in Telegram." }
                    div { class: "source-dialog-actions",
                        button { class: "source-secondary-action", r#type: "button", onclick: move |_| confirm_change.set(false), "Cancel" }
                        button { class: "source-primary-action", r#type: "button", onclick: move |_| { confirm_change.set(false); dispatch(SourceCommand::ChangeAccount, error); }, "Sign out and change" }
                    }
                }
            }
        }
    }
}

#[component]
fn SourceRow(chat: SourceChat, compact: bool) -> Element {
    let error = use_signal(|| None::<String>);
    let chat_id = chat.chat_id;
    let selected = chat.selected;
    let action = if selected { "Remove" } else { "Add" };
    let icon = match chat.kind {
        SourceKind::SavedMessages => "bookmark",
        SourceKind::PersonalChannel => "channel",
        SourceKind::MusicBot => "bot",
        SourceKind::OtherChat => "chat",
    };
    rsx! {
        div { class: if compact { "source-row compact" } else { "source-row" },
            div { class: "source-avatar {icon}", aria_hidden: "true",
                if chat.kind == SourceKind::SavedMessages {
                    svg { view_box: "0 0 24 24", fill: "none", path { d: "M6 3h12v18l-6-4-6 4V3Z" } }
                } else if chat.kind == SourceKind::PersonalChannel {
                    svg { view_box: "0 0 24 24", fill: "none", path { d: "m3 10 18-7-5 18-4-6-5 3-1-7-3-1Zm4 1 11-6-6 10" } }
                } else if chat.kind == SourceKind::MusicBot {
                    svg { view_box: "0 0 24 24", fill: "none", rect { x: "4", y: "7", width: "16", height: "12", rx: "4" } circle { cx: "9", cy: "13", r: "1" } circle { cx: "15", cy: "13", r: "1" } path { d: "M12 4v3m-4 9h8" } }
                } else {
                    svg { view_box: "0 0 24 24", fill: "none", path { d: "M4 5h16v12H9l-5 3V5Z" } }
                }
            }
            div { class: "source-row-copy", strong { dir: "auto", "{chat.title}" } span { dir: "auto", "{chat.subtitle}" } }
            button { class: if selected { "source-choice selected" } else { "source-choice" },
                r#type: "button", aria_label: "{action} {chat.title}", aria_pressed: selected,
                onclick: move |_| dispatch(SourceCommand::SetSelected { chat_id, selected: !selected }, error),
                if compact {
                    svg { view_box: "0 0 24 24", fill: "none",
                        if selected { path { d: "m5 12 4 4L19 6" } }
                        else { path { d: "M12 5v14M5 12h14" } }
                    }
                }
                else { span { class: "choice-thumb" } }
            }
        }
        if let Some(message) = error.read().as_ref() { p { class: "source-message", role: "alert", "{message}" } }
    }
}
