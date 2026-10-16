use dioxus::prelude::*;
use music_core::domain::{LibrarySnapshot, TrackKey};

const NOTE: Asset = asset!("/assets/music-note.svg");

#[component]
pub fn HomeView(
    snapshot: LibrarySnapshot,
    on_open_settings: EventHandler<()>,
    on_see_all: EventHandler<()>,
    on_play: EventHandler<TrackKey>,
) -> Element {
    rsx! {
        main { class: "home-screen",
            header { class: "home-header",
                h1 { "Home" }
                button {
                    class: "home-settings",
                    r#type: "button",
                    aria_label: "Settings",
                    title: "Settings",
                    onclick: move |_| on_open_settings.call(()),
                    svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "M10 3h4l.6 2.3a7.8 7.8 0 0 1 1.7 1l2.3-.7 2 3.5-1.7 1.7a8.2 8.2 0 0 1 0 2l1.7 1.7-2 3.5-2.3-.7a7.8 7.8 0 0 1-1.7 1L14 21h-4l-.6-2.3a7.8 7.8 0 0 1-1.7-1l-2.3.7-2-3.5 1.7-1.7a8.2 8.2 0 0 1 0-2L3.4 9.5l2-3.5 2.3.7a7.8 7.8 0 0 1 1.7-1L10 3Z" }
                        circle { cx: "12", cy: "12", r: "2.5" }
                    }
                }
            }

            if let Some(error) = &snapshot.error {
                p { class: "home-error", role: "alert", "{error}" }
            }

            section { class: "home-section home-recent", aria_label: "Recently added",
                div { class: "home-section-heading",
                    h2 { "Recently Added" }
                    if !snapshot.recent_tracks.is_empty() {
                        button { class: "home-see-all", r#type: "button", onclick: move |_| on_see_all.call(()),
                            "See all"
                            svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round",
                                path { d: "m9 5 7 7-7 7" }
                            }
                        }
                    }
                }
                if snapshot.recent_tracks.is_empty() {
                    div { class: "home-empty home-empty-recent",
                        strong { if snapshot.indexing_sources > 0 { "Finding your tracks" } else { "No tracks yet" } }
                        p { if snapshot.indexing_sources > 0 { "Music from your sources will appear as it is indexed." } else { "Choose music sources to bring your Telegram tracks here." } }
                    }
                } else {
                    ul { class: "home-carousel", aria_label: "Recently added tracks",
                        for track in &snapshot.recent_tracks {
                            li { class: "home-media-card", key: "{track.chat_id}-{track.message_id}",
                                div { class: "home-media-art",
                                    if let Some(cover) = &track.cover_data {
                                        img { src: "data:image/jpeg;base64,{cover}", alt: "" }
                                    } else {
                                        img { class: "home-art-fallback", src: NOTE, alt: "" }
                                    }
                                }
                                strong { dir: "auto", "{track.title}" }
                                span { dir: "auto", "{track.artist}" }
                            }
                        }
                    }
                }
            }

            section { class: "home-section home-listening", aria_label: "Continue listening",
                div { class: "home-section-heading", h2 { "Continue Listening" } }
                if snapshot.recent_plays.is_empty() {
                    div { class: "home-empty home-empty-compact",
                        p { "Songs you play will appear here." }
                    }
                } else {
                    ul { class: "home-listening-list", aria_label: "Recently played tracks",
                        for track in &snapshot.recent_plays {
                            li { key: "played-{track.chat_id}-{track.message_id}",
                                button { class: "home-listening-track", r#type: "button",
                                    aria_label: format!("Play {} by {}", track.title, track.artist),
                                    onclick: { let key = TrackKey { account_id: snapshot.account_id.unwrap_or_default(), chat_id: track.chat_id, message_id: track.message_id }; move |_| on_play.call(key) },
                                    span { class: "home-listening-art",
                                        if let Some(cover) = &track.cover_data { img { src: "data:image/jpeg;base64,{cover}", alt: "" } }
                                        else { img { src: NOTE, alt: "" } }
                                    }
                                    span { class: "home-listening-copy", strong { dir: "auto", "{track.title}" } small { dir: "auto", "{track.artist}" } }
                                    svg { view_box: "0 0 24 24", fill: "currentColor", path { d: "M8 5v14l11-7z" } }
                                }
                            }
                        }
                    }
                }
            }

            section { class: "home-section home-playlists", aria_label: "Your playlists",
                div { class: "home-section-heading", h2 { "Your Playlists" } }
                div { class: "home-empty home-empty-compact",
                    p { "Your playlists will appear here when playlist creation is ready." }
                }
            }
        }
    }
}
