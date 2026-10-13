use crate::runtime::AuthService;
use dioxus::prelude::*;
use music_core::domain::{StorageCommand, StorageSnapshot, TrackKey};

pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let units = ["KB", "MB", "GB", "TB"];
    let mut value = bytes as f64 / 1024.0;
    let mut index = 0;
    while value >= 1024.0 && index < units.len() - 1 {
        value /= 1024.0;
        index += 1;
    }
    format!("{value:.1} {}", units[index])
}

fn send(command: StorageCommand, mut error: Signal<Option<String>>) {
    error.set(None);
    if let Err(message) = AuthService::global().and_then(|service| service.submit_storage(command))
    {
        error.set(Some(message));
    }
}

#[component]
pub fn StorageView(snapshot: StorageSnapshot, on_back: EventHandler<()>) -> Element {
    let mut confirm_delete = use_signal(|| None::<(TrackKey, String)>);
    let mut confirm_clear = use_signal(|| false);
    let error = use_signal(|| None::<String>);
    let total = snapshot
        .app_bytes
        .max(snapshot.download_bytes.saturating_add(snapshot.cache_bytes));
    let downloaded_percent = if total > 0 {
        snapshot.download_bytes.saturating_mul(100) / total
    } else {
        0
    };
    let cache_percent = if total > 0 {
        snapshot.cache_bytes.saturating_mul(100) / total
    } else {
        0
    };
    let clearable = snapshot.download_bytes.saturating_add(snapshot.cache_bytes);
    rsx! {
        main { class: "utility-screen", aria_label: "Storage management",
            div { class: "utility-content",
                header { class: "utility-header",
                    button { class: "utility-back", r#type: "button", aria_label: "Back to Settings",
                        onclick: move |_| on_back.call(()),
                        svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round", path { d: "m15 18-6-6 6-6" } }
                    }
                    h1 { "Storage management" }
                    button { class: "storage-refresh", r#type: "button", aria_label: "Refresh storage sizes",
                        onclick: move |_| send(StorageCommand::Refresh, error),
                        svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round", path { d: "M20 11a8 8 0 1 0-2.4 6.7M20 4v7h-7" } }
                    }
                }
                p { class: "utility-intro", "See what TuneStash keeps on this device." }
                section { class: "storage-overview", aria_label: "Storage used by TuneStash",
                    div { class: "storage-total", span { "App storage" } strong { "{format_bytes(total)}" } }
                    div { class: "storage-meter", role: "img", aria_label: format!("{} offline downloads, {} Telegram media cache, {} other app data", format_bytes(snapshot.download_bytes), format_bytes(snapshot.cache_bytes), format_bytes(snapshot.other_bytes)),
                        span { class: "storage-meter-downloads", style: "width: {downloaded_percent}%" }
                        span { class: "storage-meter-cache", style: "width: {cache_percent}%" }
                    }
                    dl { class: "storage-breakdown",
                        div { dt { span { class: "storage-dot downloads" } "Offline downloads" } dd { "{format_bytes(snapshot.download_bytes)} · {snapshot.download_count} songs" } }
                        div { dt { span { class: "storage-dot cache" } "Telegram media cache" } dd { "{format_bytes(snapshot.cache_bytes)}" } }
                        div { dt { span { class: "storage-dot other" } "Other app data" } dd { "{format_bytes(snapshot.other_bytes)}" } }
                    }
                }
                section { class: "storage-mode", aria_label: "Streaming preference",
                    label { class: "storage-switch-row",
                        span { strong { "Stream only" } small { "Prevent new offline saves. Existing downloads stay and play locally." } }
                        input { r#type: "checkbox", role: "switch", checked: snapshot.stream_only,
                            aria_label: "Stream only", onchange: move |event| send(StorageCommand::SetStreamOnly(event.checked()), error) }
                    }
                    p { "Streaming temporarily stores audio while it plays. TuneStash clears it afterward." }
                }
                section { class: "storage-downloads", aria_label: "Saved songs",
                    div { class: "storage-section-heading", h2 { "Saved songs" } span { "{snapshot.download_count}" } }
                    if snapshot.downloads.is_empty() {
                        p { class: "storage-empty", "No songs saved for offline listening." }
                    } else {
                        ul { class: "storage-song-list",
                            for song in &snapshot.downloads {
                                li { key: "{song.key.chat_id}-{song.key.message_id}",
                                    span { class: "storage-song-copy",
                                        strong { dir: "auto", if song.title.is_empty() { "Saved audio" } else { "{song.title}" } }
                                        small { dir: "auto", if song.artist.is_empty() { "Telegram audio" } else { "{song.artist}" } }
                                    }
                                    span { class: "storage-song-size", "{format_bytes(song.size_bytes)}" }
                                    button { class: "storage-delete", r#type: "button",
                                        aria_label: format!("Delete saved copy of {}", if song.title.is_empty() { "Saved audio" } else { &song.title }),
                                        onclick: { let key = song.key; let title = song.title.clone(); move |_| confirm_delete.set(Some((key, title.clone()))) },
                                        svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round", path { d: "M3 6h18m-2 0-1 15H6L5 6m3 0V4h8v2m-6 4v7m4-7v7" } }
                                    }
                                }
                            }
                        }
                        if snapshot.has_more { button { class: "storage-more", r#type: "button", onclick: move |_| send(StorageCommand::LoadMore, error), "Show more songs" } }
                    }
                }
                if let Some(message) = error.read().as_ref().or(snapshot.error.as_ref()) {
                    p { class: "storage-error", role: "alert", "{message}" }
                }
                button { class: "storage-clear", r#type: "button", disabled: clearable == 0 || snapshot.busy,
                    onclick: move |_| confirm_clear.set(true), "Clear downloaded audio and cache" }
                p { class: "storage-clear-note", "Your library, settings and Telegram messages will stay." }
            }
        }
        if let Some((key, title)) = confirm_delete.read().clone() {
            div { class: "storage-dialog-backdrop", role: "presentation",
                div { class: "storage-dialog", role: "alertdialog", aria_modal: "true", aria_labelledby: "storage-delete-title", aria_describedby: "storage-delete-description",
                    h2 { id: "storage-delete-title", "Remove saved copy?" }
                    p { id: "storage-delete-description", dir: "auto",
                        if title.is_empty() { "This song" } else { "{title}" }
                        " will need Telegram to play again. Its message and library entry will stay."
                    }
                    div { class: "storage-dialog-actions",
                        button { r#type: "button", onclick: move |_| confirm_delete.set(None), "Cancel" }
                        button { class: "danger", r#type: "button", onclick: move |_| { confirm_delete.set(None); send(StorageCommand::DeleteOffline(key), error); }, "Remove download" }
                    }
                }
            }
        }
        if *confirm_clear.read() {
            div { class: "storage-dialog-backdrop", role: "presentation",
                div { class: "storage-dialog", role: "alertdialog", aria_modal: "true", aria_labelledby: "storage-clear-title", aria_describedby: "storage-clear-description",
                    h2 { id: "storage-clear-title", "Clear audio storage?" }
                    p { id: "storage-clear-description", "Remove {snapshot.download_count} saved songs and up to {format_bytes(clearable)} of audio and cache. Playback will stop. Your library and Telegram messages will stay." }
                    div { class: "storage-dialog-actions",
                        button { r#type: "button", onclick: move |_| confirm_clear.set(false), "Cancel" }
                        button { class: "danger", r#type: "button", onclick: move |_| { confirm_clear.set(false); send(StorageCommand::ClearAudio, error); }, "Clear audio" }
                    }
                }
            }
        }
    }
}
