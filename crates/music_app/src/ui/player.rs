use crate::runtime::AuthService;
use dioxus::prelude::*;
use music_core::domain::{
    DownloadStatus, PlaybackCommand, PlaybackSnapshot, PlaybackStatus, RepeatMode, StorageCommand,
};

const NOTE: Asset = asset!("/assets/music-note.svg");

fn send(command: PlaybackCommand) {
    if let Ok(service) = AuthService::global() {
        let _ = service.submit_playback(command);
    }
}

fn clock(ms: i64) -> String {
    let seconds = ms.max(0) / 1000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    Song,
    Queue,
}

#[component]
pub fn PlayerView(
    snapshot: PlaybackSnapshot,
    stream_only: bool,
    on_close: EventHandler<()>,
) -> Element {
    let mut panel = use_signal(|| Panel::Song);
    let mut saved_actions = use_signal(|| None::<(music_core::domain::TrackKey, String)>);
    let mut confirm_remove = use_signal(|| None::<(music_core::domain::TrackKey, String)>);
    let Some(track) = snapshot.current().cloned() else {
        return rsx! {};
    };
    let cover = snapshot.artwork_data.clone().or_else(|| {
        track
            .cover_data
            .as_ref()
            .map(|data| format!("data:image/jpeg;base64,{data}"))
    });
    let duration = snapshot
        .duration_ms
        .or(track.duration_seconds.map(|s| s * 1000))
        .unwrap_or(0);
    let remaining = (duration - snapshot.position_ms).max(0);
    let seek_percent = if duration > 0 {
        (snapshot.position_ms.saturating_mul(100) / duration).clamp(0, 100)
    } else {
        0
    };
    let is_playing = matches!(
        snapshot.status,
        PlaybackStatus::Playing | PlaybackStatus::Buffering | PlaybackStatus::Loading
    );
    let status = match snapshot.status {
        PlaybackStatus::Loading => "Opening audio…",
        PlaybackStatus::Buffering => "Buffering…",
        PlaybackStatus::Failed => "Playback stopped",
        _ => "",
    };
    rsx! {
        main { class: "player-screen", aria_label: "Now playing",
            header { class: "player-header",
                button { class: "player-icon-button", r#type: "button", aria_label: "Close player", onclick: move |_| on_close.call(()),
                    svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", path { d: "m5 9 7 7 7-7" } }
                }
                div { class: "player-tabs", role: "tablist", aria_label: "Player view",
                    button { role: "tab", class: if *panel.read() == Panel::Song { "selected" } else { "" }, aria_selected: *panel.read() == Panel::Song, onclick: move |_| panel.set(Panel::Song), "Song" }
                    button { role: "tab", class: if *panel.read() == Panel::Queue { "selected" } else { "" }, aria_selected: *panel.read() == Panel::Queue, onclick: move |_| panel.set(Panel::Queue), "Queue" }
                }
                button { class: "player-icon-button", r#type: "button", disabled: true, aria_label: "More actions unavailable",
                    svg { view_box: "0 0 24 24", fill: "currentColor", circle { cx: "5", cy: "12", r: "1.3" } circle { cx: "12", cy: "12", r: "1.3" } circle { cx: "19", cy: "12", r: "1.3" } }
                }
            }
            if *panel.read() == Panel::Song {
                div { class: "player-song-body",
                    div { class: "player-cover-wrap",
                        if let Some(cover) = cover { img { class: "player-cover", src: cover, alt: "" } }
                        else { img { class: "player-cover player-cover-fallback", src: NOTE, alt: "" } }
                    }
                    div { class: "player-track-line",
                        div { class: "player-track-copy",
                            h1 { dir: "auto", "{track.title}" }
                            p { dir: "auto", "{track.artist}" }
                        }
                        button { class: if snapshot.favorite { "player-heart active" } else { "player-heart" },
                            r#type: "button", aria_label: if snapshot.favorite { "Remove from favorites" } else { "Add to favorites" },
                            aria_pressed: snapshot.favorite, onclick: move |_| send(PlaybackCommand::ToggleFavorite),
                            svg { view_box: "0 0 24 24", fill: "currentColor", path { d: "M12 21s-8.5-5.4-9.7-10.7C1.4 6.3 3.7 3 7.4 3c2 0 3.5 1 4.6 2.3C13.1 4 14.6 3 16.6 3c3.7 0 6 3.3 5.1 7.3C20.5 15.6 12 21 12 21Z" } }
                        }
                    }
                    div { class: "player-progress",
                        input { r#type: "range", min: "0", max: "{duration.max(1)}", value: "{snapshot.position_ms.min(duration).max(0)}",
                            style: "--seek-pct: {seek_percent}%",
                            disabled: duration == 0, aria_label: "Seek position",
                            oninput: move |event| if let Ok(ms) = event.value().parse::<i64>() { send(PlaybackCommand::Seek(ms)); }
                        }
                        div { span { "{clock(snapshot.position_ms)}" } span { "-{clock(remaining)}" } }
                    }
                    div { class: "player-transport",
                        button { class: if snapshot.shuffle { "active" } else { "" }, r#type: "button", aria_label: "Shuffle", aria_pressed: snapshot.shuffle, onclick: move |_| send(PlaybackCommand::ToggleShuffle),
                            svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round", path { d: "M4 6h2c4 0 7 12 11 12h3m-3-3 3 3-3 3M4 18h2c1.5 0 2.6-1.6 3.7-3.6M14.3 9.6C15.4 7.6 16.5 6 18 6h2m-3-3 3 3-3 3" } }
                        }
                        button { r#type: "button", aria_label: "Previous track", onclick: move |_| send(PlaybackCommand::Previous),
                            svg { view_box: "0 0 24 24", fill: "currentColor", path { d: "M4 4h3v16H4zM20 4v16L8 12z" } }
                        }
                        button { class: "player-play", r#type: "button", aria_label: if is_playing { "Pause" } else { "Play" },
                            onclick: move |_| send(if is_playing { PlaybackCommand::Pause } else { PlaybackCommand::Play }),
                            if is_playing { svg { view_box: "0 0 24 24", fill: "currentColor", rect { x: "6", y: "4", width: "4", height: "16", rx: "1" } rect { x: "14", y: "4", width: "4", height: "16", rx: "1" } } }
                            else { svg { view_box: "0 0 24 24", fill: "currentColor", path { d: "M7 4.5v15l12-7.5z" } } }
                        }
                        button { r#type: "button", aria_label: "Next track", onclick: move |_| send(PlaybackCommand::Next),
                            svg { view_box: "0 0 24 24", fill: "currentColor", path { d: "M17 4h3v16h-3zM4 4v16l12-8z" } }
                        }
                        button { class: if snapshot.repeat != RepeatMode::Off { "active" } else { "" }, r#type: "button",
                            aria_label: match snapshot.repeat { RepeatMode::Off => "Repeat off", RepeatMode::Queue => "Repeat queue", RepeatMode::Single => "Single play" },
                            title: match snapshot.repeat { RepeatMode::Off => "Repeat off", RepeatMode::Queue => "Repeat queue", RepeatMode::Single => "Single play" },
                            onclick: move |_| send(PlaybackCommand::CycleRepeat),
                            svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round", path { d: "m17 2 3 3-3 3M20 5H7a4 4 0 0 0-4 4v2m4 11-3-3 3-3m-3 3h13a4 4 0 0 0 4-4v-2" } }
                            if snapshot.repeat == RepeatMode::Single { span { class: "player-single-mark", "1" } }
                        }
                    }
                    if !status.is_empty() { p { class: "player-status", role: "status", "{status}" } }
                    if let Some(error) = &snapshot.error { p { class: "player-error", role: "alert", "{error}" } }
                    div { class: "player-actions",
                        button { r#type: "button", disabled: stream_only && snapshot.download != DownloadStatus::Saved,
                            aria_label: if snapshot.download == DownloadStatus::Saved { "Saved song actions" } else if stream_only { "Download unavailable while Stream only is on" } else { "Download song" },
                            onclick: { let key = track.key; let title = track.title.clone(); move |_| if snapshot.download == DownloadStatus::Saved { saved_actions.set(Some((key, title.clone()))); } else { send(PlaybackCommand::Download); } },
                            svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round",
                                path { d: if snapshot.download == DownloadStatus::Saved { "m5 12 4 4 10-10" } else { "M12 3v12m0 0 4-4m-4 4-4-4M4 18v3h16v-3" } }
                            }
                            span { if stream_only && snapshot.download != DownloadStatus::Saved { "Stream only" } else { match snapshot.download { DownloadStatus::Remote => "Download", DownloadStatus::Downloading => "Saving…", DownloadStatus::Saved => "Saved", DownloadStatus::Failed => "Retry save" } } }
                        }
                        button { r#type: "button", disabled: true, aria_label: "Add to playlist unavailable", svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", path { d: "M3 6h13M3 11h13M3 16h9m5-2v7m-3.5-3.5h7" } } span { "Add to Playlist" } }
                        button { r#type: "button", disabled: true, aria_label: "Go to album unavailable", svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", circle { cx: "12", cy: "12", r: "9" } circle { cx: "12", cy: "12", r: "2" } } span { "Go to Album" } }
                        button { r#type: "button", disabled: true, aria_label: "Share unavailable", svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", path { d: "M12 16V3m0 0L8 7m4-4 4 4M5 12v8h14v-8" } } span { "Share" } }
                    }
                }
            } else {
                div { class: "player-queue-body",
                    h1 { "Playing from Library" }
                    p { "{snapshot.queue.len()} tracks · Tap a track to play" }
                    ol { class: "player-queue-list",
                        for (position, index) in snapshot.order.iter().enumerate() {
                            if let Some(item) = snapshot.queue.get(*index) {
                                li { key: "{item.key.chat_id}-{item.key.message_id}",
                                    button { class: if position == snapshot.cursor { "current" } else { "" }, r#type: "button",
                                        aria_label: format!("Play {} by {}", item.title, item.artist),
                                        onclick: { let key = item.key; move |_| send(PlaybackCommand::Select(key)) },
                                        span { class: "queue-number", if position == snapshot.cursor { svg { view_box: "0 0 24 24", fill: "currentColor", path { d: "M10 4v12.2A3.2 3.2 0 1 1 8 13.3V6l11-2v9.2a3.2 3.2 0 1 1-2-2.9V2z" } } } else { "{position + 1}" } }
                                        span { class: "queue-copy", strong { dir: "auto", "{item.title}" } small { dir: "auto", "{item.artist}" } }
                                        span { class: "queue-duration", "{clock(item.duration_seconds.unwrap_or(0) * 1000)}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some((key, title)) = saved_actions.read().clone() {
            div { class: "storage-dialog-backdrop", role: "presentation",
                div { class: "storage-dialog player-saved-sheet", role: "dialog", aria_modal: "true", aria_labelledby: "saved-actions-title",
                    h2 { id: "saved-actions-title", "Saved for offline" }
                    p { "This song is stored on this device." }
                    button { class: "player-remove-action", r#type: "button", onclick: move |_| { saved_actions.set(None); confirm_remove.set(Some((key, title.clone()))); },
                        svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round", path { d: "M3 6h18m-2 0-1 15H6L5 6m3 0V4h8v2m-6 4v7m4-7v7" } }
                        "Remove download"
                    }
                    button { class: "player-sheet-cancel", r#type: "button", onclick: move |_| saved_actions.set(None), "Cancel" }
                }
            }
        }
        if let Some((key, title)) = confirm_remove.read().clone() {
            div { class: "storage-dialog-backdrop", role: "presentation",
                div { class: "storage-dialog", role: "alertdialog", aria_modal: "true", aria_labelledby: "player-remove-title", aria_describedby: "player-remove-description",
                    h2 { id: "player-remove-title", "Remove saved copy?" }
                    p { id: "player-remove-description", dir: "auto", "{title} will need Telegram to play again. Playback will stop if this song is selected. Its library entry and message will stay." }
                    div { class: "storage-dialog-actions",
                        button { r#type: "button", onclick: move |_| confirm_remove.set(None), "Cancel" }
                        button { class: "danger", r#type: "button", onclick: move |_| {
                            confirm_remove.set(None);
                            if let Ok(service) = AuthService::global() { let _ = service.submit_storage(StorageCommand::DeleteOffline(key)); }
                        }, "Remove download" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn MiniPlayer(snapshot: PlaybackSnapshot, on_open: EventHandler<()>) -> Element {
    let Some(track) = snapshot.current() else {
        return rsx! {};
    };
    let playing = matches!(
        snapshot.status,
        PlaybackStatus::Playing | PlaybackStatus::Loading | PlaybackStatus::Buffering
    );
    rsx! {
        div { class: "mini-player",
            button { class: "mini-player-open", r#type: "button", onclick: move |_| on_open.call(()),
                if let Some(cover) = snapshot.artwork_data.clone().or_else(|| track.cover_data.as_ref().map(|data| format!("data:image/jpeg;base64,{data}"))) { img { src: cover, alt: "" } }
                else { img { src: NOTE, alt: "" } }
                span { strong { dir: "auto", "{track.title}" } small { dir: "auto", "{track.artist}" } }
            }
            button { class: "mini-player-toggle", r#type: "button", aria_label: if playing { "Pause" } else { "Play" },
                onclick: move |_| send(if playing { PlaybackCommand::Pause } else { PlaybackCommand::Play }),
                if playing { svg { view_box: "0 0 24 24", fill: "currentColor", rect { x: "6", y: "4", width: "4", height: "16", rx: "1" } rect { x: "14", y: "4", width: "4", height: "16", rx: "1" } } }
                else { svg { view_box: "0 0 24 24", fill: "currentColor", path { d: "M7 4.5v15l12-7.5z" } } }
            }
        }
    }
}
