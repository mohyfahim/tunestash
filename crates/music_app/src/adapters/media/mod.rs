//! Android Media3 bridge. Rust owns queue policy; the Android service owns the player.

#[cfg(target_os = "android")]
mod android {
    use crate::runtime::AuthService;
    use jni::{
        JNIEnv, JavaVM,
        objects::{GlobalRef, JObject, JString, JValue},
        sys::{jbyteArray, jlong},
    };
    use music_core::domain::TrackKey;
    use std::sync::OnceLock;

    static SERVICE_CONTEXT: OnceLock<(JavaVM, GlobalRef)> = OnceLock::new();

    pub fn with_context<T>(
        f: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        if let Some((vm, context)) = SERVICE_CONTEXT.get() {
            let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
            return f(&mut env, context.as_obj());
        }
        let context = ndk_context::android_context();
        let vm = unsafe { JavaVM::from_raw(context.vm().cast()) }.map_err(|e| e.to_string())?;
        let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
        let activity = unsafe { JObject::from_raw(context.context().cast()) };
        f(&mut env, &activity)
    }

    pub fn dispatch(action: &str, payload: &str) -> Result<(), String> {
        with_context(|env, context| {
            let class = env.get_object_class(context).map_err(|e| e.to_string())?;
            let action = env.new_string(action).map_err(|e| e.to_string())?;
            let payload = env.new_string(payload).map_err(|e| e.to_string())?;
            env.call_static_method(
                class,
                "playerCommand",
                "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)V",
                &[
                    JValue::Object(context),
                    JValue::Object(&action),
                    JValue::Object(&payload),
                ],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        })
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_dev_dioxus_main_PlayerService_nativeBootstrap(
        env: JNIEnv,
        service: JObject,
    ) {
        if SERVICE_CONTEXT.get().is_none()
            && let (Ok(vm), Ok(context)) = (env.get_java_vm(), env.new_global_ref(service))
        {
            let _ = SERVICE_CONTEXT.set((vm, context));
        }
        let _ = AuthService::global();
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_dev_dioxus_main_PlayerService_nativeReadRange(
        env: JNIEnv,
        _service: JObject,
        account: jlong,
        chat: jlong,
        message: jlong,
        offset: jlong,
        length: jlong,
    ) -> jbyteArray {
        let key = TrackKey {
            account_id: account,
            chat_id: chat,
            message_id: message,
        };
        let Ok(bytes) = AuthService::global()
            .and_then(|service| service.read_range(key, offset, length.clamp(0, 262_144) as usize))
        else {
            return std::ptr::null_mut();
        };
        match env.byte_array_from_slice(&bytes) {
            Ok(array) => array.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_dev_dioxus_main_PlayerService_nativeEvent(
        mut env: JNIEnv,
        _service: JObject,
        raw: JString,
    ) {
        if let Ok(raw) = env.get_string(&raw)
            && let Ok(value) = serde_json::from_str(raw.to_str().unwrap_or(""))
            && let Ok(service) = AuthService::global()
        {
            let _ = service.submit_media_event(value);
        }
    }
}

#[cfg(target_os = "android")]
pub use android::{dispatch, with_context};

#[cfg(not(target_os = "android"))]
pub fn dispatch(_action: &str, _payload: &str) -> Result<(), String> {
    Err("Playback is available in the Android app.".into())
}
