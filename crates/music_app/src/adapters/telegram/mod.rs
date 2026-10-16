//! TDLib C/JSON client. Only this module touches unsafe FFI.

#[cfg(any(target_os = "android", test))]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod library;
#[cfg(any(target_os = "android", test))]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod playback;
#[cfg(any(target_os = "android", test))]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod sources;

#[cfg(all(test, not(target_os = "android")))]
#[allow(dead_code)]
struct TdJson;

#[cfg(all(test, not(target_os = "android")))]
#[allow(dead_code)]
impl TdJson {
    fn send(&self, _value: &Value) -> Result<(), String> {
        Ok(())
    }

    fn next_request_id(&self) -> i64 {
        static NEXT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
    }
}

#[cfg(target_os = "android")]
use music_core::domain::AuthSnapshot;
#[cfg(any(target_os = "android", test))]
use music_core::domain::{AuthCommand, AuthStage};
#[cfg(target_os = "android")]
use music_core::domain::{LibrarySnapshot, SourceCommand, SourceSnapshot};
#[cfg(any(target_os = "android", test))]
use serde_json::{Value, json};
#[cfg(target_os = "android")]
use std::time::{Duration, Instant};

#[cfg(target_os = "android")]
use crate::runtime::DriverCommand;
#[cfg(target_os = "android")]
use std::{
    ffi::{CStr, CString, c_char},
    path::PathBuf,
    sync::mpsc::Receiver,
};
#[cfg(target_os = "android")]
use tokio::sync::watch;

/// Convert Telegram's errors to user-facing recovery messages without showing secrets.
#[cfg(any(target_os = "android", test))]
fn auth_error(message: &str) -> String {
    if message.contains("PHONE_NUMBER_INVALID") {
        "Enter a valid phone number with its country code.".into()
    } else if message.contains("PHONE_CODE_INVALID") {
        "That code is incorrect. Check it and try again.".into()
    } else if message.contains("PHONE_CODE_EXPIRED") {
        "That code expired. Request a new one.".into()
    } else if message.contains("PASSWORD_HASH_INVALID") {
        "That password is incorrect. Try again.".into()
    } else if message.contains("EMAIL_CODE_INVALID") {
        "That email code is incorrect. Try again.".into()
    } else if message.contains("API_ID_INVALID") || message.contains("API_ID_PUBLISHED_FLOOD") {
        "This app build cannot connect with its Telegram credentials.".into()
    } else if message.contains("FLOOD_WAIT") {
        "Telegram asked you to wait before trying again.".into()
    } else if message.contains("NETWORK") || message.contains("Timeout") {
        "Can't reach Telegram. Check your connection and retry.".into()
    } else {
        "Telegram couldn't complete that step. Check your details and retry.".into()
    }
}

#[cfg(any(target_os = "android", test))]
fn phone_is_valid(phone: &str) -> bool {
    let digits = phone.strip_prefix('+').unwrap_or("");
    (5..=15).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(any(target_os = "android", test))]
fn playable_music_message(message: &Value) -> bool {
    let content = &message["content"];
    if content["@type"] == "messageAudio" {
        return true;
    }
    if content["@type"] != "messageDocument" {
        return false;
    }
    let document = &content["document"];
    let mime = document["mime_type"]
        .as_str()
        .unwrap_or("")
        .to_ascii_lowercase();
    let name = document["file_name"]
        .as_str()
        .unwrap_or("")
        .to_ascii_lowercase();
    mime.starts_with("audio/")
        || [".mp3", ".m4a", ".aac", ".flac", ".ogg", ".opus", ".wav"]
            .iter()
            .any(|extension| name.ends_with(extension))
}

#[cfg(any(target_os = "android", test))]
enum MusicProbePage {
    Found(i64),
    Next(i64),
    Exhausted,
    Invalid(&'static str),
}

#[cfg(any(target_os = "android", test))]
fn music_probe_page(value: &Value, before: i64) -> MusicProbePage {
    let Some(messages) = value["messages"].as_array() else {
        return MusicProbePage::Invalid("Telegram returned an invalid message page");
    };
    if let Some(message) = messages
        .iter()
        .find(|message| playable_music_message(message))
    {
        return message["id"].as_i64().map_or(
            MusicProbePage::Invalid("Telegram returned a message without an ID"),
            MusicProbePage::Found,
        );
    }
    match value["next_from_message_id"].as_i64() {
        Some(0) => MusicProbePage::Exhausted,
        Some(next) if next > 0 && next != before => MusicProbePage::Next(next),
        Some(_) => MusicProbePage::Invalid("Telegram repeated a pagination cursor"),
        None => MusicProbePage::Invalid("Telegram returned no pagination cursor"),
    }
}

#[cfg(any(target_os = "android", test))]
fn command_request(stage: &AuthStage, command: AuthCommand) -> Result<Value, String> {
    match (stage, command) {
        (_, AuthCommand::Retry) => Ok(json!({"@type":"getAuthorizationState"})),
        (AuthStage::Phone | AuthStage::Code { .. }, AuthCommand::Phone(phone))
            if phone_is_valid(&phone) =>
        {
            Ok(json!({
                "@type":"setAuthenticationPhoneNumber", "phone_number":phone,
                "settings":null
            }))
        }
        (AuthStage::Phone | AuthStage::Code { .. }, AuthCommand::Phone(_)) => {
            Err("Enter your phone number as +country code and number.".into())
        }
        (AuthStage::Code { .. }, AuthCommand::Code(code)) if !code.trim().is_empty() => {
            Ok(json!({"@type":"checkAuthenticationCode", "code":code.trim()}))
        }
        (
            AuthStage::Code {
                resend_after: Some(0),
                ..
            },
            AuthCommand::ResendCode,
        ) => Ok(json!({"@type":"resendAuthenticationCode", "reason":null})),
        (AuthStage::EmailCode { .. }, AuthCommand::ResendCode) => {
            Ok(json!({"@type":"resendAuthenticationCode", "reason":null}))
        }
        (AuthStage::EmailAddress, AuthCommand::EmailAddress(email))
            if email.contains('@') && !email.contains(' ') =>
        {
            Ok(json!({"@type":"setAuthenticationEmailAddress", "email_address":email.trim()}))
        }
        (AuthStage::EmailCode { .. }, AuthCommand::EmailCode(code)) if !code.trim().is_empty() => {
            Ok(json!({
                "@type":"checkAuthenticationEmailCode",
                "code":{"@type":"emailAddressAuthenticationCode", "code":code.trim()}
            }))
        }
        (AuthStage::Password { .. }, AuthCommand::Password(password)) if !password.is_empty() => {
            Ok(json!({"@type":"checkAuthenticationPassword", "password":password}))
        }
        _ => Err("Complete the current Telegram sign-in step first.".into()),
    }
}

#[cfg(target_os = "android")]
type CreateClient = unsafe extern "C" fn() -> i32;
#[cfg(target_os = "android")]
type Send = unsafe extern "C" fn(i32, *const c_char);
#[cfg(target_os = "android")]
type Receive = unsafe extern "C" fn(f64) -> *const c_char;
#[cfg(target_os = "android")]
type Execute = unsafe extern "C" fn(*const c_char) -> *const c_char;

#[cfg(target_os = "android")]
struct TdJson {
    _library: libloading::Library,
    client_id: i32,
    send: Send,
    receive: Receive,
    next_extra: std::cell::Cell<i64>,
}

#[cfg(target_os = "android")]
impl TdJson {
    fn load() -> Result<Self, String> {
        let library = unsafe { libloading::Library::new("libtdjson.so") }
            .map_err(|_| "Telegram library is missing from this build.".to_string())?;
        // Function pointers remain valid because this struct owns the library.
        let create: CreateClient =
            *unsafe { library.get::<CreateClient>(b"td_create_client_id\0") }
                .map_err(|_| "Telegram library is incompatible with this build.".to_string())?;
        let send: Send = *unsafe { library.get::<Send>(b"td_send\0") }
            .map_err(|_| "Telegram library is incompatible with this build.".to_string())?;
        let receive: Receive = *unsafe { library.get::<Receive>(b"td_receive\0") }
            .map_err(|_| "Telegram library is incompatible with this build.".to_string())?;
        let execute: Execute = *unsafe { library.get::<Execute>(b"td_execute\0") }
            .map_err(|_| "Telegram library is incompatible with this build.".to_string())?;
        let quiet_logs =
            CString::new(r#"{"@type":"setLogVerbosityLevel","new_verbosity_level":1}"#)
                .expect("static TDLib command has no NUL");
        let _ = unsafe { execute(quiet_logs.as_ptr()) };
        let client_id = unsafe { create() };
        Ok(Self {
            _library: library,
            client_id,
            send,
            receive,
            next_extra: std::cell::Cell::new(0),
        })
    }

    fn send(&self, value: &Value) -> Result<(), String> {
        let request =
            CString::new(value.to_string()).map_err(|_| "Invalid Telegram request".to_string())?;
        unsafe { (self.send)(self.client_id, request.as_ptr()) };
        Ok(())
    }

    fn next_request_id(&self) -> i64 {
        let next = self.next_extra.get() + 1;
        self.next_extra.set(next);
        next
    }

    fn receive(&self) -> Option<Value> {
        let raw = unsafe { (self.receive)(0.25) };
        if raw.is_null() {
            return None;
        }
        // TDLib owns this buffer and may invalidate it on the next receive.
        let bytes = unsafe { CStr::from_ptr(raw) }.to_bytes().to_vec();
        serde_json::from_slice(&bytes).ok()
    }
}

#[cfg(target_os = "android")]
pub struct TelegramConfig {
    pub api_id: i32,
    pub api_hash: String,
    pub database_dir: String,
    pub files_dir: String,
    pub database_key: Vec<u8>,
    pub source_db: String,
}

#[cfg(target_os = "android")]
struct Driver {
    td: TdJson,
    config: TelegramConfig,
    snapshot: AuthSnapshot,
    updates: watch::Sender<AuthSnapshot>,
    pending: Option<i64>,
    last_phone: String,
    resend_deadline: Option<Instant>,
    sources: sources::SourceEngine,
    library: library::LibraryEngine,
    playback: playback::PlaybackEngine,
    logout_pending: Option<i64>,
}

#[cfg(target_os = "android")]
impl Driver {
    fn logout_marker(&self) -> PathBuf {
        std::path::Path::new(&self.config.source_db).with_file_name("logout.pending")
    }

    fn finish_logout(&mut self) -> Result<(), String> {
        let account = self.logout_pending.ok_or("No pending account cleanup")?;
        self.playback.finish_logout(account)?;
        self.sources.clear_account_data(account)?;
        for directory in [&self.config.database_dir, &self.config.files_dir] {
            match std::fs::remove_dir_all(directory) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(format!("Could not clear Telegram storage: {error}")),
            }
        }
        crate::adapters::secure_store::clear_tdlib_key()?;
        self.config.database_key = crate::adapters::secure_store::android_storage()?.database_key;
        std::fs::create_dir_all(&self.config.database_dir)
            .map_err(|error| format!("Could not prepare Telegram storage: {error}"))?;
        std::fs::create_dir_all(&self.config.files_dir)
            .map_err(|error| format!("Could not prepare Telegram storage: {error}"))?;
        match std::fs::remove_file(self.logout_marker()) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Could not finish local cleanup: {error}")),
        }
        self.td = TdJson::load()?;
        self.td
            .send(&json!({"@type":"getAuthorizationState", "@extra":-2}))?;
        self.sources.signed_out();
        self.library.reset();
        self.logout_pending = None;
        self.snapshot.stage = AuthStage::Connecting;
        self.snapshot.error = None;
        self.publish();
        Ok(())
    }

    fn publish(&self) {
        self.updates.send_replace(self.snapshot.clone());
    }

    fn send_auth(&mut self, mut request: Value) -> Result<(), String> {
        let extra = self.td.next_request_id();
        request["@extra"] = json!(extra);
        self.td.send(&request)?;
        self.pending = Some(extra);
        self.snapshot.busy = true;
        self.snapshot.error = None;
        self.publish();
        Ok(())
    }

    fn handle_command(&mut self, command: AuthCommand) {
        if command == AuthCommand::RetryCleanup {
            if matches!(self.snapshot.stage, AuthStage::CleanupFailed { .. }) {
                self.snapshot.stage = AuthStage::Connecting;
                self.publish();
                if let Err(error) = self.finish_logout() {
                    self.snapshot.stage = AuthStage::CleanupFailed { description: error };
                    self.publish();
                }
            }
            return;
        }
        if self.logout_pending.is_some() {
            return;
        }
        if self.snapshot.busy {
            return;
        }
        if let AuthCommand::Phone(phone) = &command {
            self.last_phone = phone.clone();
        }
        match command_request(&self.snapshot.stage, command) {
            Ok(request) => {
                if let Err(error) = self.send_auth(request) {
                    self.snapshot.error = Some(error);
                    self.publish();
                }
            }
            Err(error) => {
                self.snapshot.error = Some(error);
                self.publish();
            }
        }
    }

    fn handle_value(&mut self, value: Value) {
        if value["@type"] == "updateAuthorizationState" {
            self.handle_state(&value["authorization_state"]);
            return;
        }
        self.library.observe_update(&value);
        if self.playback.handle_value(&self.td, &value) {
            return;
        }
        if self.sources.handle_value(&self.td, &value) {
            if self.logout_pending.is_some() && !self.sources.snapshot().signing_out {
                self.logout_pending = None;
                let _ = std::fs::remove_file(self.logout_marker());
            }
            return;
        }
        if self.library.handle_response(&value) {
            return;
        }
        let extra = value["@extra"].as_i64();
        if extra.is_some() && extra == self.pending {
            self.pending = None;
            self.snapshot.busy = false;
            if value["@type"] == "error" {
                self.snapshot.error = Some(auth_error(value["message"].as_str().unwrap_or("")));
            }
            self.publish();
        } else if extra == Some(-1) {
            if value["@type"] == "user" {
                if let Some(user_id) = value["id"].as_i64() {
                    self.snapshot.stage = AuthStage::Ready { user_id };
                    self.snapshot.busy = false;
                    self.snapshot.error = None;
                    self.publish();
                    self.sources.authorized(&self.td, user_id, &value);
                }
            } else {
                self.snapshot.error = Some(
                    "Telegram authorized the session, but account details could not be loaded."
                        .into(),
                );
                self.snapshot.busy = false;
                self.publish();
            }
        } else if value["@type"]
            .as_str()
            .is_some_and(|kind| kind.starts_with("authorizationState"))
        {
            self.handle_state(&value);
        }
    }

    fn handle_state(&mut self, state: &Value) {
        if self.logout_pending.is_some()
            && matches!(self.snapshot.stage, AuthStage::CleanupFailed { .. })
        {
            return;
        }
        let kind = state["@type"].as_str().unwrap_or("");
        let client_closed = kind == "authorizationStateClosed";
        if self.logout_pending.is_some() && !client_closed {
            self.snapshot.stage = AuthStage::Connecting;
            self.publish();
            return;
        }
        self.pending = None;
        self.snapshot.busy = false;
        self.snapshot.error = None;
        self.resend_deadline = None;
        self.snapshot.stage = match kind {
            "authorizationStateWaitTdlibParameters" => {
                use base64::{Engine as _, engine::general_purpose::STANDARD};
                let request = json!({
                    "@type":"setTdlibParameters", "use_test_dc":false,
                    "database_directory":self.config.database_dir,
                    "files_directory":self.config.files_dir,
                    "database_encryption_key":STANDARD.encode(&self.config.database_key),
                    "use_file_database":true, "use_chat_info_database":true,
                    "use_message_database":true, "use_secret_chats":false,
                    "api_id":self.config.api_id, "api_hash":self.config.api_hash,
                    "system_language_code":"en", "device_model":"Android",
                    "system_version":"Android", "application_version":env!("CARGO_PKG_VERSION")
                });
                if let Err(error) = self.send_auth(request) { self.snapshot.error = Some(error); }
                AuthStage::Connecting
            }
            "authorizationStateWaitPhoneNumber" => AuthStage::Phone,
            "authorizationStateWaitCode" => {
                let info = &state["code_info"];
                if let Some(phone) = info["phone_number"].as_str() {
                    self.last_phone = phone.to_string();
                }
                let delivery = match info["type"]["@type"].as_str().unwrap_or("") {
                    "authenticationCodeTypeTelegramMessage" => "Telegram app",
                    "authenticationCodeTypeSms" => "SMS",
                    "authenticationCodeTypeCall" => "phone call",
                    "authenticationCodeTypeEmailAddress" => "email",
                    _ => "the delivery method Telegram selected",
                };
                let timeout = if info["next_type"].is_null() { None } else { Some(info["timeout"].as_u64().unwrap_or(0) as u32) };
                self.resend_deadline = timeout.map(|s| Instant::now() + Duration::from_secs(u64::from(s)));
                AuthStage::Code { phone: self.last_phone.clone(), delivery: delivery.into(), resend_after: timeout }
            }
            "authorizationStateWaitEmailAddress" => AuthStage::EmailAddress,
            "authorizationStateWaitEmailCode" => AuthStage::EmailCode {
                address_pattern: state["code_info"]["email_address_pattern"].as_str().unwrap_or("your email").into()
            },
            "authorizationStateWaitPassword" => AuthStage::Password {
                hint: state["password_hint"].as_str().unwrap_or("").into()
            },
            "authorizationStateReady" => {
                let _ = self.td.send(&json!({"@type":"getMe", "@extra":-1}));
                AuthStage::Connecting
            }
            "authorizationStateWaitRegistration" => AuthStage::Unsupported {
                description:"This build cannot create a new Telegram account. Complete registration in Telegram, then return here.".into()
            },
            "authorizationStateWaitOtherDeviceConfirmation" => AuthStage::Unsupported {
                description:"This account requires confirmation on another device. Complete it in Telegram, then retry here.".into()
            },
            "authorizationStateClosing" | "authorizationStateClosed" | "authorizationStateLoggingOut" => AuthStage::Connecting,
            _ => AuthStage::Unsupported { description:"Telegram requested an authentication step this version does not support.".into() },
        };
        self.publish();
        if matches!(self.snapshot.stage, AuthStage::Phone) && self.logout_pending.is_none() {
            self.sources.signed_out();
            self.library.reset();
        }
        if client_closed {
            if self.logout_pending.is_some() {
                if let Err(error) = self.finish_logout() {
                    self.snapshot.stage = AuthStage::CleanupFailed { description: error };
                    self.publish();
                }
                return;
            }
            self.sources.signed_out();
            self.library.reset();
            self.playback.set_account(None);
            match TdJson::load() {
                Ok(td) => {
                    self.td = td;
                    if self
                        .td
                        .send(&json!({"@type":"getAuthorizationState", "@extra":-2}))
                        .is_err()
                    {
                        self.snapshot.error =
                            Some("Couldn't start a new Telegram session. Restart the app.".into());
                        self.publish();
                    }
                }
                Err(error) => {
                    self.snapshot.error = Some(error);
                    self.publish();
                }
            }
        }
    }

    fn tick(&mut self) {
        let Some(deadline) = self.resend_deadline else {
            return;
        };
        if let AuthStage::Code { resend_after, .. } = &mut self.snapshot.stage {
            let seconds = deadline.saturating_duration_since(Instant::now()).as_secs() as u32;
            if *resend_after != Some(seconds) {
                *resend_after = Some(seconds);
                self.publish();
            }
        }
    }
}

#[cfg(target_os = "android")]
pub fn run(
    config: TelegramConfig,
    commands: Receiver<DriverCommand>,
    updates: watch::Sender<AuthSnapshot>,
    source_updates: watch::Sender<SourceSnapshot>,
    library_updates: watch::Sender<LibrarySnapshot>,
    search_updates: watch::Sender<music_core::domain::SearchSnapshot>,
    playback_updates: watch::Sender<music_core::domain::PlaybackSnapshot>,
    storage_updates: watch::Sender<music_core::domain::StorageSnapshot>,
) {
    let td = match TdJson::load() {
        Ok(td) => td,
        Err(error) => {
            updates.send_modify(|state| state.error = Some(error));
            return;
        }
    };
    let sources = sources::SourceEngine::new(&config.source_db, source_updates);
    let library = library::LibraryEngine::new(&config.source_db, library_updates, search_updates);
    let playback = playback::PlaybackEngine::new(
        &config.source_db,
        &config.files_dir,
        playback_updates,
        storage_updates,
    );
    let mut driver = Driver {
        td,
        config,
        snapshot: AuthSnapshot::default(),
        updates,
        pending: None,
        last_phone: String::new(),
        resend_deadline: None,
        sources,
        library,
        playback,
        logout_pending: None,
    };
    match std::fs::read_to_string(driver.logout_marker()) {
        Ok(contents) => match contents.trim().parse::<i64>() {
            Ok(account) => {
                driver.logout_pending = Some(account);
                if let Err(error) = driver.finish_logout() {
                    driver.snapshot.stage = AuthStage::CleanupFailed { description: error };
                    driver.publish();
                }
            }
            Err(_) => {
                driver.snapshot.stage = AuthStage::Unsupported {
                    description:
                        "Local sign-out state is damaged. Clear this app's data to continue.".into(),
                };
                driver.publish();
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let _ = driver
                .td
                .send(&json!({"@type":"getAuthorizationState", "@extra":-2}));
        }
        Err(error) => {
            driver.snapshot.stage = AuthStage::Unsupported {
                description: format!("Cannot check local sign-out state: {error}"),
            };
            driver.publish();
        }
    }
    loop {
        while let Ok(command) = commands.try_recv() {
            match command {
                DriverCommand::Auth(command) => driver.handle_command(command),
                DriverCommand::Source(SourceCommand::ChangeAccount) => {
                    if let Some(account) = driver.sources.snapshot().account_id {
                        if let Err(error) =
                            std::fs::write(driver.logout_marker(), account.to_string())
                        {
                            driver
                                .sources
                                .logout_error(&format!("Cannot prepare local cleanup: {error}"));
                            continue;
                        }
                        driver.logout_pending = Some(account);
                        driver.playback.begin_logout(&driver.td);
                        driver.sources.change_account(&driver.td);
                        if !driver.sources.snapshot().signing_out {
                            driver.logout_pending = None;
                            let _ = std::fs::remove_file(driver.logout_marker());
                        }
                    }
                }
                DriverCommand::Source(command) => {
                    driver.sources.handle_command(&driver.td, command)
                }
                DriverCommand::Library(command) => driver.library.command(command),
                DriverCommand::Search(command) => driver.library.search_command(command),
                DriverCommand::Playback(command) => {
                    driver
                        .playback
                        .command(&driver.td, &driver.library, command)
                }
                DriverCommand::Storage(command) => {
                    driver.playback.storage_command(&driver.td, command)
                }
                DriverCommand::ReadRange {
                    key,
                    offset,
                    length,
                    response,
                } => driver
                    .playback
                    .read_range(&driver.td, key, offset, length, response),
                DriverCommand::MediaEvent(value) => {
                    if driver.playback.media_event(&driver.td, &value) {
                        driver.library.refresh_recent_plays();
                    }
                }
            }
        }
        if let Some(value) = driver.td.receive() {
            driver.handle_value(value);
        }
        driver.tick();
        driver.sources.drive(&driver.td);
        driver.library.sync_sources(driver.sources.snapshot());
        let sources = driver.sources.snapshot();
        driver.playback.set_account(
            sources
                .account_id
                .filter(|_| sources.setup_complete && !sources.signing_out),
        );
        driver.library.drive(&driver.td);
        driver.playback.reconcile(&driver.library);
        driver.playback.tick(&driver.td);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn music_detection_accepts_any_sender_and_excludes_non_audio() {
        assert!(playable_music_message(
            &json!({"sender_id":{"user_id":7}, "content":{"@type":"messageAudio"}}),
        ));
        assert!(playable_music_message(
            &json!({"sender_id":{"user_id":8}, "content":{"@type":"messageAudio"}}),
        ));
        assert!(playable_music_message(
            &json!({"sender_id":{"user_id":7}, "content":{"@type":"messageDocument", "document":{"mime_type":"audio/flac", "file_name":"track.bin"}}}),
        ));
        assert!(playable_music_message(
            &json!({"content":{"@type":"messageDocument", "document":{"mime_type":"application/octet-stream", "file_name":"song.mp3"}}}),
        ));
        assert!(!playable_music_message(
            &json!({"sender_id":{"user_id":7}, "content":{"@type":"messageVoiceNote"}}),
        ));
        assert!(!playable_music_message(
            &json!({"content":{"@type":"messageDocument", "document":{"mime_type":"application/pdf", "file_name":"notes.pdf"}}}),
        ));
        assert!(!playable_music_message(
            &json!({"content":{"@type":"messageText", "text":{"text":"https://example.com/song.mp3"}}}),
        ));
    }

    #[test]
    fn music_probe_uses_tdlib_cursor_until_history_is_exhausted() {
        let first = json!({"messages":[{"id":500,"content":{"@type":"messageDocument", "document":{"mime_type":"application/pdf", "file_name":"notes.pdf"}}}],"next_from_message_id":400});
        assert!(matches!(
            music_probe_page(&first, 0),
            MusicProbePage::Next(400)
        ));
        let last = json!({"messages":[],"next_from_message_id":0});
        assert!(matches!(
            music_probe_page(&last, 400),
            MusicProbePage::Exhausted
        ));
        let found = json!({"messages":[{"id":350,"content":{"@type":"messageAudio"}}],"next_from_message_id":200});
        assert!(matches!(
            music_probe_page(&found, 400),
            MusicProbePage::Found(350)
        ));
        assert!(matches!(
            music_probe_page(&first, 400),
            MusicProbePage::Invalid("Telegram repeated a pagination cursor")
        ));
    }

    #[test]
    fn phone_requires_international_format() {
        assert!(
            command_request(
                &AuthStage::Phone,
                AuthCommand::Phone("+989121234567".into())
            )
            .is_ok()
        );
        assert!(
            command_request(&AuthStage::Phone, AuthCommand::Phone("09121234567".into())).is_err()
        );
    }

    #[test]
    fn password_command_only_on_password_stage() {
        assert!(
            command_request(
                &AuthStage::Password {
                    hint: String::new()
                },
                AuthCommand::Password("secret".into())
            )
            .is_ok()
        );
        assert!(
            command_request(
                &AuthStage::Code {
                    phone: String::new(),
                    delivery: String::new(),
                    resend_after: None
                },
                AuthCommand::Password("secret".into())
            )
            .is_err()
        );
    }

    #[test]
    fn telegram_errors_do_not_show_protocol_text() {
        assert_eq!(
            auth_error("PHONE_CODE_INVALID"),
            "That code is incorrect. Check it and try again."
        );
    }

    #[test]
    fn email_code_can_be_resent_at_telegram_request() {
        let request = command_request(
            &AuthStage::EmailCode {
                address_pattern: "a***@example.com".into(),
            },
            AuthCommand::ResendCode,
        )
        .unwrap();
        assert_eq!(request["@type"], "resendAuthenticationCode");
    }
}
