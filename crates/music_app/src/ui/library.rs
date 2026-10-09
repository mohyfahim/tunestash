use crate::runtime::AuthService;
use crate::ui::home::HomeView;
use dioxus::prelude::*;
use music_core::domain::{LibraryCommand, LibrarySnapshot, LibrarySort, TrackSummary};

const NOTE: Asset = asset!("/assets/music-note.svg");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Home,
    Search,
    Library,
}

#[component]
fn TrackRow(track: TrackSummary) -> Element {
    let cover = track
        .cover_data
        .as_ref()
        .map(|data| format!("data:image/jpeg;base64,{data}"));
    rsx! {
        li { class: "library-track", key: "{track.chat_id}-{track.message_id}",
            div { class: "library-art",
                if let Some(cover) = cover {
                    img { src: cover, alt: "" }
                } else {
                    img { src: NOTE, alt: "" }
                }
            }
            div { class: "library-track-copy",
                strong { dir: "auto", "{track.title}" }
                span { dir: "auto", "{track.artist} · {track.source_name}" }
            }
            button { class: "library-row-action", r#type: "button", disabled: true, aria_label: "Download unavailable",
                svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round",
                    path { d: "M12 3v11m0 0 4-4m-4 4-4-4M5 16v3h14v-3" }
                }
            }
            button { class: "library-row-action", r#type: "button", disabled: true, aria_label: "Track options unavailable",
                svg { view_box: "0 0 24 24", fill: "currentColor",
                    circle { cx: "5", cy: "12", r: "1.5" }
                    circle { cx: "12", cy: "12", r: "1.5" }
                    circle { cx: "19", cy: "12", r: "1.5" }
                }
            }
        }
    }
}

#[component]
pub fn LibraryView(snapshot: LibrarySnapshot, on_open_sources: EventHandler<()>) -> Element {
    let mut tab = use_signal(|| Tab::Home);
    let is_library = *tab.read() == Tab::Library;
    let current_sort = snapshot.sort;
    rsx! {
        div { class: "music-shell",
            if is_library {
                main { class: "library-screen",
                    header { class: "library-header",
                        h1 { "Library" }
                        button {
                            class: "library-reindex",
                            r#type: "button",
                            aria_label: if snapshot.indexing_sources > 0 { "Indexing selected sources" } else { "Reindex selected sources" },
                            title: if snapshot.indexing_sources > 0 { "Indexing selected sources" } else { "Reindex selected sources" },
                            disabled: snapshot.selected_sources == 0 || snapshot.indexing_sources > 0 || snapshot.error.is_some(),
                            onclick: move |_| {
                                if let Ok(service) = AuthService::global() {
                                    let _ = service.submit_library(LibraryCommand::Reindex);
                                }
                            },
                            svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round",
                                path { d: "M20 11a8 8 0 1 0-2.4 6.7M20 4v7h-7" }
                            }
                        }
                    }
                    div { class: "library-pills", aria_label: "Library categories",
                        span { class: "library-pill selected", "Tracks" }
                        button { class: "library-pill", r#type: "button", disabled: true, "Playlists" }
                        button { class: "library-pill", r#type: "button", disabled: true, "Favorites" }
                        button { class: "library-pill", r#type: "button", disabled: true, "Artists" }
                        button { class: "library-pill", r#type: "button", disabled: true, "Albums" }
                    }
                    div { class: "library-toolbar",
                        span { class: "library-count",
                            if snapshot.total_count == 1 { "1 track" } else { "{snapshot.total_count} tracks" }
                            if current_sort == LibrarySort::Recent { " · Recent first" }
                        }
                        div { class: "library-tools",
                            button { r#type: "button", disabled: true, class: "library-tool downloaded",
                                svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", path { d: "M12 3v11m0 0 4-4m-4 4-4-4M5 16a8 8 0 1 0 14-6" } }
                                "Downloaded"
                            }
                            button { r#type: "button", disabled: true, class: "library-tool icon-only", aria_label: "Audio filter unavailable",
                                svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round", path { d: "M5 12V8a7 7 0 0 1 14 0v4M5 12h2v7H5a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2Zm14 0h-2v7h2a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2Z" } }
                            }
                            button {
                                r#type: "button",
                                class: if current_sort == LibrarySort::Recent { "library-tool icon-only sort-active" } else { "library-tool icon-only" },
                                aria_label: if current_sort == LibrarySort::Recent { "Sort tracks by title" } else { "Sort tracks by most recent" },
                                title: if current_sort == LibrarySort::Recent { "Sort by title" } else { "Sort by recent" },
                                onclick: move |_| {
                                    if let Ok(service) = AuthService::global() {
                                        let next = if current_sort == LibrarySort::Recent { LibrarySort::Title } else { LibrarySort::Recent };
                                        let _ = service.submit_library(LibraryCommand::SetSort(next));
                                    }
                                },
                                svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", path { d: "M4 7h16M7 12h10M10 17h4" } }
                            }
                        }
                    }
                    if let Some(error) = &snapshot.error {
                        div { class: "library-alert", role: "alert",
                            span { "{error}" }
                            button { r#type: "button", onclick: move |_| { if let Ok(service) = AuthService::global() { let _ = service.submit_library(LibraryCommand::Retry); } }, "Retry" }
                        }
                    }
                    div { class: "library-scroll",
                        if snapshot.tracks.is_empty() && snapshot.active_initial.is_none() {
                            div { class: "library-empty",
                                img { src: NOTE, alt: "" }
                                h2 { if snapshot.indexing_sources > 0 { "Finding your tracks" } else { "Your library is empty" } }
                                p { if snapshot.indexing_sources > 0 { "Music from your selected Telegram sources will appear here as it is indexed." } else { "Tracks from your selected Telegram sources will appear here." } }
                            }
                        } else {
                            div { class: "library-list-wrap",
                                if snapshot.tracks.is_empty() {
                                    div { class: "library-empty library-empty-filter",
                                        h2 { "No tracks start with {snapshot.active_initial.unwrap_or_default()}" }
                                        p { "Tap the highlighted letter again to show all tracks." }
                                    }
                                } else {
                                    ul { class: "library-list",
                                        for track in snapshot.tracks.iter().cloned() {
                                            TrackRow { track }
                                        }
                                    }
                                }
                                if snapshot.sort == LibrarySort::Title {
                                    div { class: "library-alphabet", aria_label: "Filter tracks by first letter",
                                        for letter in "ABCDEFGHIJKLMNOPQRSTUVWXYZ".chars() {
                                            button {
                                                class: if snapshot.active_initial == Some(letter) { "library-letter active" } else { "library-letter" },
                                                r#type: "button",
                                                aria_label: if snapshot.active_initial == Some(letter) { format!("Clear {letter} filter") } else { format!("Show tracks starting with {letter}") },
                                                aria_pressed: snapshot.active_initial == Some(letter),
                                                onclick: move |_| { if let Ok(service) = AuthService::global() { let _ = service.submit_library(LibraryCommand::SelectInitial(letter)); } },
                                                "{letter}"
                                            }
                                        }
                                    }
                                }
                            }
                            if snapshot.has_more {
                                button { class: "library-more", r#type: "button", onclick: move |_| { if let Ok(service) = AuthService::global() { let _ = service.submit_library(LibraryCommand::LoadMore); } }, "Show more tracks" }
                            }
                        }
                        if snapshot.indexing_sources > 0 {
                            p { class: "library-indexing", role: "status", "Indexing {snapshot.indexing_sources} selected source(s)…" }
                        }
                    }
                }
            } else if *tab.read() == Tab::Home {
                HomeView {
                    snapshot: snapshot.clone(),
                    on_open_sources: move |_| on_open_sources.call(()),
                    on_see_all: move |_| {
                        if let Ok(service) = AuthService::global() {
                            let _ = service.submit_library(LibraryCommand::SetSort(LibrarySort::Recent));
                        }
                        tab.set(Tab::Library);
                    },
                }
            } else {
                main { class: "music-black-page", aria_label: "Search placeholder" }
            }
            nav { class: "music-bottom-nav", aria_label: "Main navigation",
                button { class: if *tab.read() == Tab::Home { "nav-tab active" } else { "nav-tab" }, r#type: "button", onclick: move |_| tab.set(Tab::Home),
                    svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round", path { d: "m3 10 9-7 9 7v10a1 1 0 0 1-1 1h-5v-7H9v7H4a1 1 0 0 1-1-1V10Z" } }
                    span { "Home" }
                }
                button { class: if *tab.read() == Tab::Search { "nav-tab active" } else { "nav-tab" }, r#type: "button", onclick: move |_| tab.set(Tab::Search),
                    svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", circle { cx: "10.5", cy: "10.5", r: "7.5" } path { d: "m16 16 5 5" } }
                    span { "Search" }
                }
                button { class: if is_library { "nav-tab active" } else { "nav-tab" }, r#type: "button", onclick: move |_| {
                    if let Ok(service) = AuthService::global() {
                        let _ = service.submit_library(LibraryCommand::SetSort(LibrarySort::Title));
                    }
                    tab.set(Tab::Library);
                },
                    svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round", path { d: "M4 4h16v16H4zM4 9h16M8 13h8M8 17h5" } }
                    span { "Library" }
                }
            }
        }
    }
}
