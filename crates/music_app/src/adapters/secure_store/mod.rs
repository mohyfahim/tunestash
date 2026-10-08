//! Android-only private storage and TDLib database key access.

#[cfg(target_os = "android")]
pub struct AndroidStorage {
    pub files_dir: String,
    pub database_key: Vec<u8>,
}

#[cfg(target_os = "android")]
pub fn android_storage() -> Result<AndroidStorage, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use jni::{
        JavaVM,
        objects::{JObject, JString, JValue},
    };

    let context = ndk_context::android_context();
    // Dioxus owns these pointers. Borrow them before starting the TDLib thread.
    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) }
        .map_err(|_| "Android secure storage is unavailable".to_string())?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|_| "Android secure storage is unavailable".to_string())?;
    let activity = unsafe { JObject::from_raw(context.context().cast()) };
    let file = env
        .call_method(&activity, "getFilesDir", "()Ljava/io/File;", &[])
        .and_then(|v| v.l())
        .map_err(|_| "Cannot access private app storage".to_string())?;
    let path = env
        .call_method(&file, "getAbsolutePath", "()Ljava/lang/String;", &[])
        .and_then(|v| v.l())
        .map_err(|_| "Cannot access private app storage".to_string())?;
    let path = JString::from(path);
    let files_dir: String = env
        .get_string(&path)
        .map_err(|_| "Cannot access private app storage".to_string())?
        .into();
    // This runs on a Rust worker thread. Looking up an app class by name here
    // uses the bootstrap class loader, so resolve it from the activity object.
    let activity_class = env
        .get_object_class(&activity)
        .map_err(|_| "Android secure storage is unavailable".to_string())?;
    let key = env
        .call_static_method(
            activity_class,
            "getOrCreateTdlibKey",
            "(Landroid/content/Context;)Ljava/lang/String;",
            &[JValue::Object(&activity)],
        )
        .and_then(|v| v.l())
        .map_err(|_| "Android Keystore could not unlock TuneStash data".to_string())?;
    let key = JString::from(key);
    let key_string: String = env
        .get_string(&key)
        .map_err(|_| "Android Keystore could not unlock TuneStash data".to_string())?
        .into();
    let database_key = STANDARD
        .decode(key_string)
        .map_err(|_| "Android Keystore returned an invalid key".to_string())?;
    if database_key.len() != 32 {
        return Err("Android Keystore returned an invalid key".into());
    }
    Ok(AndroidStorage {
        files_dir,
        database_key,
    })
}
