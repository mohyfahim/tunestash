package dev.dioxus.main

import android.content.Context
import android.graphics.Color
import android.os.Bundle
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.webkit.WebView
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

typealias BuildConfig = com.tunestash.app.BuildConfig

class MainActivity : WryActivity() {
    override fun onWebViewCreate(webView: WebView) {
        webView.setBackgroundColor(Color.rgb(23, 26, 25))
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        window.statusBarColor = Color.rgb(23, 26, 25)
        window.navigationBarColor = Color.BLACK
        @Suppress("DEPRECATION")
        window.decorView.systemUiVisibility = 0
        super.onCreate(savedInstanceState)
    }

    companion object {
        private const val ALIAS = "tunestash.tdlib.database.v1"
        private const val PREFS = "tunestash.private.keys"
        private const val KEY = "tdlib_key_ciphertext"

        @JvmStatic
        fun getOrCreateTdlibKey(context: Context): String {
            val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
            val secret = if (store.containsAlias(ALIAS)) {
                store.getKey(ALIAS, null) as SecretKey
            } else {
                val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
                generator.init(
                    KeyGenParameterSpec.Builder(
                        ALIAS,
                        KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
                    ).setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                        .build()
                )
                generator.generateKey()
            }
            val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            val encoded = prefs.getString(KEY, null)
            val raw = if (encoded == null) {
                val newKey = ByteArray(32).also { SecureRandom().nextBytes(it) }
                val cipher = Cipher.getInstance("AES/GCM/NoPadding")
                cipher.init(Cipher.ENCRYPT_MODE, secret)
                val stored = cipher.iv + cipher.doFinal(newKey)
                check(prefs.edit().putString(KEY, Base64.encodeToString(stored, Base64.NO_WRAP)).commit())
                newKey
            } else {
                val stored = Base64.decode(encoded, Base64.NO_WRAP)
                require(stored.size > 12)
                val cipher = Cipher.getInstance("AES/GCM/NoPadding")
                cipher.init(Cipher.DECRYPT_MODE, secret, GCMParameterSpec(128, stored.copyOfRange(0, 12)))
                cipher.doFinal(stored.copyOfRange(12, stored.size))
            }
            require(raw.size == 32)
            return Base64.encodeToString(raw, Base64.NO_WRAP)
        }
    }
}
