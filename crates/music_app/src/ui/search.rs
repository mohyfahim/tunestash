use crate::runtime::AuthService;
use dioxus::prelude::*;
use music_core::{
    application::normalize_search,
    domain::{SearchCommand, SearchSnapshot, TrackKey, TrackSummary},
};

const NOTE: Asset = asset!("/assets/music-note.svg");

fn send(command: SearchCommand, mut error: Signal<Option<String>>) {
    error.set(None);
    if let Err(message) = AuthService::global().and_then(|service| service.submit_search(command)) {
        error.set(Some(message));
    }
}

#[component]
fn SearchTrackRow(
    track: TrackSummary,
    account_id: i64,
    query: String,
    on_play: EventHandler<TrackKey>,
) -> Element {
    let normalized_query = normalize_search(&query);
    let show_filename = !normalized_query.is_empty()
        && !track.filename.is_empty()
        && normalized_query.split_whitespace().any(|term| {
            normalize_search(&track.filename).contains(term)
                && !normalize_search(&track.title).contains(term)
                && !normalize_search(&track.artist).contains(term)
        });
    let cover = track.cover_data.as_ref().map(|data| super::cover_src(data));
    let key = TrackKey {
        account_id,
        chat_id: track.chat_id,
        message_id: track.message_id,
    };
    rsx! {
        li { class: "search-track", key: "{track.chat_id}-{track.message_id}",
            button { class: "search-track-open", r#type: "button",
                aria_label: format!("Play {} by {}", track.title, track.artist),
                onclick: move |_| on_play.call(key),
                span { class: "search-art",
                    if let Some(cover) = cover { img { src: cover, alt: "" } }
                    else { img { class: "search-art-fallback", src: NOTE, alt: "" } }
                }
                span { class: "search-track-copy",
                    strong { dir: "auto", "{track.title}" }
                    small { dir: "auto", bdi { "{track.artist}" } " · " bdi { "{track.source_name}" } }
                    if show_filename { em { dir: "auto", "{track.filename}" } }
                }
            }
        }
    }
}

#[component]
pub fn SearchView(snapshot: SearchSnapshot, on_play: EventHandler<TrackKey>) -> Element {
    let mut query = use_signal(|| snapshot.query.clone());
    let local_error = use_signal(|| None::<String>);
    let typed = query.read().trim().to_string();
    let has_query = !typed.is_empty();
    let settled = typed == snapshot.query;
    let account_id = snapshot.account_id.unwrap_or_default();
    let error = local_error.read().clone().or(snapshot.error.clone());

    rsx! {
        main { class: "search-screen", aria_label: "Search music",
            div { class: "search-content",
                header { class: "search-header", h1 { "Search" } }
                form { class: "search-form", role: "search",
                    onsubmit: move |event| {
                        event.prevent_default();
                        let current = query.read().trim().to_string();
                        if !current.is_empty() {
                            send(SearchCommand::SaveQuery(current), local_error);
                        }
                    },
                    svg { class: "search-field-icon", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round",
                        circle { cx: "10.5", cy: "10.5", r: "6.5" }
                        path { d: "m15.5 15.5 5 5" }
                    }
                    input { id: "music-search", r#type: "search", value: "{query}", dir: "auto",
                        aria_label: "Search songs, artists, or files",
                        placeholder: "Search songs, artists, or files…",
                        autocomplete: "off", maxlength: "128",
                        oninput: move |event| {
                            let value = event.value();
                            query.set(value.clone());
                            send(SearchCommand::SetQuery(value), local_error);
                        }
                    }
                    if has_query {
                        button { class: "search-field-clear", r#type: "button", aria_label: "Clear search",
                            onclick: move |_| {
                                query.set(String::new());
                                send(SearchCommand::SetQuery(String::new()), local_error);
                            },
                            svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.9", stroke_linecap: "round",
                                path { d: "M6 6l12 12M18 6 6 18" }
                            }
                        }
                    }
                }

                if let Some(error) = error {
                    div { class: "search-error", role: "alert",
                        span { "{error}" }
                        button { r#type: "button", onclick: move |_| send(SearchCommand::Retry, local_error), "Retry" }
                    }
                }
                if let Some(indexing_error) = &snapshot.indexing_error {
                    div { class: "search-error", role: "alert",
                        span { "{indexing_error}" }
                        button { r#type: "button", onclick: move |_| send(SearchCommand::Retry, local_error), "Retry" }
                    }
                }

                if has_query {
                    section { class: "search-section search-results", aria_label: "Search results",
                        div { class: "search-section-heading",
                            h2 { "Results" }
                            if settled && snapshot.error.is_none() {
                                span { class: "search-result-count", role: "status", aria_live: "polite",
                                    if snapshot.total_count == 1 { "1 track" } else { "{snapshot.total_count} tracks" }
                                }
                            }
                        }
                        if !settled {
                            p { class: "search-message", role: "status", "Searching your library…" }
                        } else if snapshot.error.is_some() {
                            p { class: "search-message", "Results are unavailable. Try again." }
                        } else if snapshot.tracks.is_empty() {
                            div { class: "search-empty-results",
                                img { src: NOTE, alt: "" }
                                h3 { "No matching tracks" }
                                p { "Try a different title, artist, or filename." }
                            }
                        } else {
                            ul { class: "search-track-list",
                                for track in snapshot.tracks.iter().cloned() {
                                    SearchTrackRow { track, account_id, query: typed.clone(), on_play: move |key| {
                                        send(SearchCommand::SaveQuery(query.read().clone()), local_error);
                                        on_play.call(key);
                                    } }
                                }
                            }
                            if snapshot.has_more {
                                button { class: "search-more", r#type: "button",
                                    onclick: move |_| send(SearchCommand::LoadMore, local_error),
                                    "Show more tracks"
                                }
                            }
                        }
                    }
                } else {
                    if !snapshot.recent_queries.is_empty() {
                        section { class: "search-section", aria_label: "Recent searches",
                            div { class: "search-section-heading",
                                h2 { "Recent searches" }
                                button { class: "search-clear-history", r#type: "button",
                                    onclick: move |_| send(SearchCommand::ClearHistory, local_error), "Clear" }
                            }
                            ul { class: "search-history-list",
                                for recent in snapshot.recent_queries.iter().cloned() {
                                    li { key: "{recent}",
                                        button { r#type: "button", onclick: {
                                            let recent = recent.clone();
                                            move |_| {
                                                query.set(recent.clone());
                                                send(SearchCommand::SaveQuery(recent.clone()), local_error);
                                            }
                                        },
                                            svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round",
                                                circle { cx: "12", cy: "12", r: "8" }
                                                path { d: "M12 7v5l3 2" }
                                            }
                                            span { dir: "auto", "{recent}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    section { class: "search-section search-recent-tracks", aria_label: "Recently added tracks",
                        div { class: "search-section-heading", h2 { "Recently added" } }
                        if snapshot.recent_tracks.is_empty() {
                            div { class: "search-empty-library",
                                img { src: NOTE, alt: "" }
                                h3 { if snapshot.indexing_sources > 0 { "Finding your tracks" } else { "No tracks to search yet" } }
                                p { if snapshot.indexing_sources > 0 {
                                    "Music will appear here as your selected sources are indexed."
                                } else if snapshot.selected_sources == 0 {
                                    "Choose a music source to start building your library."
                                } else {
                                    "Tracks from your selected sources will appear here."
                                } }
                            }
                        } else {
                            ul { class: "search-track-list",
                                for track in snapshot.recent_tracks.iter().cloned() {
                                    SearchTrackRow { track, account_id, query: String::new(), on_play: move |key| on_play.call(key) }
                                }
                            }
                        }
                    }
                }
                if snapshot.indexing_sources > 0 {
                    p { class: "search-indexing", role: "status",
                        "Indexing {snapshot.indexing_sources} selected source(s)… More tracks may appear soon."
                    }
                }
            }
        }
    }
}
