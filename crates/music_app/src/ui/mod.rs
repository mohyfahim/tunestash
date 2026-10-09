//! Welcome, Telegram authentication, and music source selection.

mod sources;

use crate::runtime::AuthService;
use dioxus::prelude::*;
use music_core::{
    domain::{AuthCommand, AuthSnapshot, AuthStage, SourceSnapshot},
    ports::TelegramAuth,
};

const NOTE: Asset = asset!("/assets/music-note.svg");
const WAVE: Asset = asset!("/assets/login-wave.svg");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Phone,
    Code,
    Email,
    EmailCode,
    Password,
    Connecting,
    Unsupported,
}

fn send(command: AuthCommand, mut error: Signal<Option<String>>) {
    error.set(None);
    if let Err(message) = AuthService::global().and_then(|service| service.submit(command)) {
        error.set(Some(message));
    }
}

fn show_library(user_id: i64, sources: &SourceSnapshot, temporary_sources: Option<i64>) -> bool {
    sources.account_id == Some(user_id)
        && sources.setup_complete
        && !sources.signing_out
        && temporary_sources != Some(user_id)
}

#[component]
fn LibraryPlaceholder(on_back: EventHandler<()>) -> Element {
    rsx! {
        main { class: "library-placeholder",
            div { class: "library-placeholder-layout",
                button { class: "library-back", r#type: "button",
                    onclick: move |_| on_back.call(()),
                    "Back to Sources"
                }
                h1 { "Library" }
            }
        }
    }
}

#[allow(non_snake_case)]
pub fn App() -> Element {
    let mut started = use_signal(|| false);
    let mut edit_phone = use_signal(|| false);
    let mut snapshot = use_signal(|| match AuthService::global() {
        Ok(service) => service.snapshot(),
        Err(message) => AuthSnapshot {
            stage: AuthStage::Unsupported {
                description: message,
            },
            busy: false,
            error: None,
        },
    });
    let mut local_error = use_signal(|| None::<String>);
    let mut source_snapshot = use_signal(|| {
        AuthService::global()
            .map(|service| service.source_snapshot())
            .unwrap_or_default()
    });
    let mut temporary_sources = use_signal(|| None::<i64>);
    let mut phone = use_signal(String::new);
    let mut code = use_signal(String::new);
    let mut email = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut show_password = use_signal(|| false);

    use_future(move || async move {
        let Ok(service) = AuthService::global() else {
            return;
        };
        let mut changes = service.subscribe();
        while changes.changed().await.is_ok() {
            snapshot.set(changes.borrow_and_update().clone());
        }
    });

    use_future(move || async move {
        let Ok(service) = AuthService::global() else {
            return;
        };
        let mut changes = service.subscribe_sources();
        while changes.changed().await.is_ok() {
            source_snapshot.set(changes.borrow_and_update().clone());
        }
    });

    use_effect(move || {
        if !matches!(&snapshot.read().stage, AuthStage::Ready { .. }) {
            temporary_sources.set(None);
        }
    });

    let current = snapshot.read().clone();
    let source_state = source_snapshot.read().clone();
    let ready_user = match &current.stage {
        AuthStage::Ready { user_id } => Some(*user_id),
        _ => None,
    };
    let message = local_error.read().clone().or(current.error.clone());
    let step = if *edit_phone.read() {
        Step::Phone
    } else {
        match current.stage {
            AuthStage::Phone => Step::Phone,
            AuthStage::Code { .. } => Step::Code,
            AuthStage::EmailAddress => Step::Email,
            AuthStage::EmailCode { .. } => Step::EmailCode,
            AuthStage::Password { .. } => Step::Password,
            AuthStage::Connecting | AuthStage::Ready { .. } => Step::Connecting,
            AuthStage::Unsupported { .. } => Step::Unsupported,
        }
    };
    let (title, description, label, placeholder, input_type, autocomplete) = match &current.stage {
        _ if step == Step::Phone => (
            "Your phone number".to_string(),
            "Enter the number connected to your Telegram account.".to_string(),
            "Phone number",
            "+1 555 123 4567",
            "tel",
            "tel",
        ),
        AuthStage::Code {
            phone, delivery, ..
        } => (
            "Enter your code".into(),
            format!("Telegram sent a code to {delivery} for {phone}."),
            "Verification code",
            "Enter code",
            "text",
            "one-time-code",
        ),
        AuthStage::EmailAddress => (
            "Add your email".into(),
            "Telegram needs an email address to continue sign-in.".into(),
            "Email address",
            "you@example.com",
            "email",
            "email",
        ),
        AuthStage::EmailCode { address_pattern } => (
            "Check your email".into(),
            format!("Enter the code Telegram sent to {address_pattern}."),
            "Email code",
            "Enter code",
            "text",
            "one-time-code",
        ),
        AuthStage::Password { .. } => (
            "Two-step verification".into(),
            "Enter your Telegram password to finish connecting.".into(),
            "Telegram password",
            "Your password",
            "password",
            "current-password",
        ),
        AuthStage::Unsupported { description } => (
            "More verification needed".into(),
            description.clone(),
            "",
            "",
            "",
            "",
        ),
        _ => (
            "Connecting to Telegram".into(),
            "Checking your secure session.".into(),
            "",
            "",
            "",
            "",
        ),
    };
    let input_value = match step {
        Step::Phone => phone.read().clone(),
        Step::Code | Step::EmailCode => code.read().clone(),
        Step::Email => email.read().clone(),
        Step::Password => password.read().clone(),
        _ => String::new(),
    };
    let input_type = if step == Step::Password && *show_password.read() {
        "text"
    } else {
        input_type
    };
    let button_label = match step {
        Step::Code => "Verify code",
        Step::EmailCode => "Verify email",
        Step::Password => "Sign in",
        _ => "Continue",
    };
    let busy = current.busy;

    rsx! {
        document::Style { {include_str!("../../assets/style.css")} }
        if let Some(user_id) = ready_user {
            if show_library(user_id, &source_state, *temporary_sources.read()) {
                LibraryPlaceholder { on_back: move |_| temporary_sources.set(Some(user_id)) }
            } else {
                sources::SourceView {
                    snapshot: source_state,
                    onboarding: true,
                    on_next: move |_| temporary_sources.set(None),
                }
            }
        } else if !*started.read()
                && matches!(current.stage, AuthStage::Connecting)
                && current.error.is_none()
        {
            main { class: "signed-in-canvas", aria_label: "Connecting to Telegram" }
        } else if !*started.read() {
            main { class: "welcome-screen",
                div { class: "welcome-top",
                    div { class: "brand-mark", img { src: NOTE, alt: "" } }
                    h1 { class: "brand-wordmark", span { "Tune" } span { "Stash" } }
                    p { class: "brand-subtitle", "Your Telegram music" br {} "in one place" }
                }
                img { class: "welcome-wave", src: WAVE, alt: "" }
                div { class: "welcome-bottom",
                    button { class: "primary-button", r#type: "button", onclick: move |_| started.set(true),
                        svg { class: "telegram-icon", view_box: "0 0 24 24", path { d: "M21.7 3.3 18.4 20c-.2 1.2-.9 1.5-1.9.9l-5.2-3.8-2.5 2.4c-.3.3-.6.6-1.1.6l.4-5.3L18 5.9c.4-.4-.1-.6-.6-.3L5.1 13.4l-5-1.6c-1.1-.3-1.1-1.1.2-1.6L20 2.6c.9-.3 1.9.2 1.7.7Z" } }
                        "Continue with Telegram"
                    }
                    p { class: "privacy-copy", "Your music stays with you." br {} "We connect directly to Telegram." }
                }
            }
        } else {
            main { class: "auth-screen",
                header { class: "auth-header",
                    button { class: "back-button", r#type: "button", aria_label: "Back to welcome", onclick: move |_| { started.set(false); edit_phone.set(false); local_error.set(None); },
                        svg { view_box: "0 0 24 24", path { d: "m15 18-6-6 6-6" } }
                    }
                    span { class: "auth-brand", "Tune" span { "Stash" } }
                }
                div { class: "auth-content",
                    div { class: "auth-mark", img { src: NOTE, alt: "" } }
                    h1 { "{title}" }
                    p { class: "auth-description", "{description}" }
                    if matches!(step, Step::Phone | Step::Code | Step::Email | Step::EmailCode | Step::Password) {
                        form { onsubmit: move |event| {
                            event.prevent_default();
                            let command = match step {
                                Step::Phone => { edit_phone.set(false); AuthCommand::Phone(phone.read().trim().to_string()) },
                                Step::Code => AuthCommand::Code(code.read().clone()),
                                Step::Email => AuthCommand::EmailAddress(email.read().clone()),
                                Step::EmailCode => AuthCommand::EmailCode(code.read().clone()),
                                Step::Password => AuthCommand::Password(password.read().clone()),
                                _ => return,
                            };
                            send(command, local_error);
                            if matches!(step, Step::Code | Step::EmailCode) { code.set(String::new()); }
                            if step == Step::Password { password.set(String::new()); }
                        },
                            label { r#for: "auth-input", "{label}" }
                            div { class: "input-wrap",
                                input { id: "auth-input", r#type: input_type, autocomplete: autocomplete, inputmode: if matches!(step, Step::Code | Step::EmailCode) { "numeric" } else { "text" }, dir: if step == Step::Email { "auto" } else { "ltr" }, placeholder: placeholder, value: input_value,
                                    oninput: move |event| match step {
                                        Step::Phone => phone.set(event.value()),
                                        Step::Code | Step::EmailCode => code.set(event.value()),
                                        Step::Email => email.set(event.value()),
                                        Step::Password => password.set(event.value()),
                                        _ => (),
                                    }
                                }
                                if step == Step::Password { button { class: "input-action", r#type: "button", aria_label: if *show_password.read() { "Hide password" } else { "Show password" }, onclick: move |_| show_password.toggle(), if *show_password.read() { "Hide" } else { "Show" } } }
                            }
                            if let AuthStage::Password { hint } = &current.stage { if !hint.is_empty() { p { class: "password-hint", "Hint: {hint}" } } }
                            button { class: "primary-button", r#type: "submit", disabled: busy, if busy { "Working…" } else { "{button_label}" } }
                        }
                    } else if step == Step::Connecting {
                        div { class: "loading-line" }
                        if message.is_some() { button { class: "text-button", onclick: move |_| send(AuthCommand::Retry, local_error), "Retry connection" } }
                    } else {
                        button { class: "primary-button", r#type: "button", onclick: move |_| send(AuthCommand::Retry, local_error), "Check again" }
                    }
                    if step == Step::Code {
                        div { class: "auth-actions",
                            button { class: "text-button", r#type: "button", onclick: move |_| { edit_phone.set(true); local_error.set(None); }, "Edit phone number" }
                            if let AuthStage::Code { resend_after: Some(seconds), .. } = current.stage {
                                button { class: "text-button", r#type: "button", disabled: seconds > 0 || busy, onclick: move |_| send(AuthCommand::ResendCode, local_error), if seconds > 0 { "Resend in {seconds}s" } else { "Resend code" } }
                            }
                        }
                    }
                    if step == Step::EmailCode { button { class: "text-button", r#type: "button", onclick: move |_| send(AuthCommand::ResendCode, local_error), "Resend email code" } }
                    if let Some(message) = message { p { class: "auth-error", role: "alert", "{message}" } }
                }
                p { class: "auth-footnote", "This is an independent Telegram session on your device." }
            }
        }
    }
}

#[cfg(test)]
mod navigation_tests {
    use super::*;

    #[test]
    fn library_is_default_after_setup_but_sources_can_open_temporarily() {
        let mut sources = SourceSnapshot {
            account_id: Some(7),
            ..Default::default()
        };
        assert!(!show_library(7, &sources, None));
        sources.setup_complete = true;
        assert!(show_library(7, &sources, None));
        assert!(!show_library(7, &sources, Some(7)));
        assert!(show_library(7, &sources, Some(8)));
        assert!(!show_library(8, &sources, None));
        sources.signing_out = true;
        assert!(!show_library(7, &sources, None));
    }
}
