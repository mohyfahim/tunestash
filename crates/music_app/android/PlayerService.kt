package dev.dioxus.main

import android.content.Context
import android.content.Intent
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.pm.ServiceInfo
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.KeyEvent
import androidx.media3.common.C
import androidx.media3.common.AudioAttributes
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.datasource.BaseDataSource
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.FileDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import androidx.media3.session.CommandButton
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionResult
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import org.json.JSONObject
import android.util.Base64
import java.io.ByteArrayOutputStream
import java.io.IOException

class PlayerService : MediaSessionService() {
    companion object {
        init { System.loadLibrary("main") }

        @JvmStatic
        fun getOrCreateTdlibKey(context: Context): String = MainActivity.getOrCreateTdlibKey(context)

        @JvmStatic
        fun playerCommand(context: Context, action: String, payload: String) {
            val intent = Intent(context, PlayerService::class.java).apply {
                putExtra("action", action)
                putExtra("payload", payload)
            }
            if (action == "play" && Build.VERSION.SDK_INT >= 26) context.startForegroundService(intent)
            else context.startService(intent)
        }
    }

    private external fun nativeBootstrap()
    private external fun nativeReadRange(account: Long, chat: Long, message: Long, offset: Long, length: Long): ByteArray?
    private external fun nativeEvent(event: String)

    private lateinit var player: ExoPlayer
    private lateinit var session: MediaSession
    private val previousCommand = SessionCommand("tunestash.previous", Bundle.EMPTY)
    private val nextCommand = SessionCommand("tunestash.next", Bundle.EMPTY)
    private val handler = Handler(Looper.getMainLooper())
    private val positionTick = object : Runnable {
        override fun run() {
            if (::player.isInitialized && player.mediaItemCount > 0) {
                nativeEvent(JSONObject().put("type", "position")
                    .put("position", player.currentPosition).toString())
            }
            handler.postDelayed(this, 1000)
        }
    }

    override fun onCreate() {
        super.onCreate()
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(NotificationChannel("tunestash_playback", "Music playback", NotificationManager.IMPORTANCE_LOW))
        val starting = Notification.Builder(this, "tunestash_playback")
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setContentTitle("TuneStash")
            .setContentText("Preparing audio")
            .setOngoing(true)
            .build()
        if (Build.VERSION.SDK_INT >= 29) startForeground(42, starting, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
        else startForeground(42, starting)
        nativeBootstrap()
        val sourceFactory = DataSource.Factory { TuneDataSource() }
        val audioAttributes = AudioAttributes.Builder()
            .setUsage(C.USAGE_MEDIA)
            .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
            .build()
        player = ExoPlayer.Builder(this)
            .setMediaSourceFactory(DefaultMediaSourceFactory(this).setDataSourceFactory(sourceFactory))
            .setAudioAttributes(audioAttributes, true)
            .setHandleAudioBecomingNoisy(true)
            .build()
        player.addListener(object : Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) {
                nativeEvent(JSONObject().put("type", if (isPlaying) "playing" else "paused").toString())
            }
            override fun onPlaybackStateChanged(state: Int) {
                val kind = when (state) {
                    Player.STATE_BUFFERING -> "buffering"
                    Player.STATE_READY -> "ready"
                    Player.STATE_ENDED -> "ended"
                    else -> null
                }
                if (kind != null) nativeEvent(JSONObject().put("type", kind)
                    .put("duration", player.duration.coerceAtLeast(0)).toString())
            }
            override fun onPlayerError(error: PlaybackException) {
                nativeEvent(JSONObject().put("type", "error")
                    .put("message", "This audio cannot be played. Check its format or connection.").toString())
            }
            override fun onMediaMetadataChanged(mediaMetadata: MediaMetadata) {
                val raw = mediaMetadata.artworkData ?: return
                val mediaId = player.currentMediaItem?.mediaId ?: return
                if (raw.size > 2_000_000) return
                val bitmap = BitmapFactory.decodeByteArray(raw, 0, raw.size) ?: return
                val scale = minOf(1f, 900f / maxOf(bitmap.width, bitmap.height))
                val resized = if (scale < 1f) Bitmap.createScaledBitmap(
                    bitmap, (bitmap.width * scale).toInt(), (bitmap.height * scale).toInt(), true
                ) else bitmap
                val output = ByteArrayOutputStream()
                resized.compress(Bitmap.CompressFormat.JPEG, 85, output)
                val jpeg = output.toByteArray()
                if (jpeg.size <= 500_000) {
                    nativeEvent(JSONObject().put("type", "artwork").put("media_id", mediaId)
                        .put("data", "data:image/jpeg;base64," + Base64.encodeToString(jpeg, Base64.NO_WRAP)).toString())
                }
            }
        })
        val previousButton = CommandButton.Builder(CommandButton.ICON_PREVIOUS)
            .setSessionCommand(previousCommand).setDisplayName("Previous")
            .setSlots(CommandButton.SLOT_BACK).build()
        val nextButton = CommandButton.Builder(CommandButton.ICON_NEXT)
            .setSessionCommand(nextCommand).setDisplayName("Next")
            .setSlots(CommandButton.SLOT_FORWARD).build()
        session = MediaSession.Builder(this, player)
            .setMediaButtonPreferences(listOf(previousButton, nextButton))
            .setCallback(object : MediaSession.Callback {
                override fun onConnect(session: MediaSession, controller: MediaSession.ControllerInfo): MediaSession.ConnectionResult {
                    val commands = MediaSession.ConnectionResult.DEFAULT_SESSION_COMMANDS.buildUpon()
                        .add(previousCommand).add(nextCommand).build()
                    return MediaSession.ConnectionResult.accept(commands, MediaSession.ConnectionResult.DEFAULT_PLAYER_COMMANDS)
                }

                override fun onCustomCommand(session: MediaSession, controller: MediaSession.ControllerInfo,
                    customCommand: SessionCommand, args: Bundle): ListenableFuture<SessionResult> {
                    val kind = when (customCommand.customAction) {
                        previousCommand.customAction -> "previous"
                        nextCommand.customAction -> "next"
                        else -> null
                    }
                    if (kind != null) nativeEvent(JSONObject().put("type", kind).toString())
                    return Futures.immediateFuture(SessionResult(if (kind != null) SessionResult.RESULT_SUCCESS else SessionResult.RESULT_ERROR_NOT_SUPPORTED))
                }

                override fun onMediaButtonEvent(session: MediaSession, controllerInfo: MediaSession.ControllerInfo,
                    intent: Intent): Boolean {
                    @Suppress("DEPRECATION")
                    val event = intent.getParcelableExtra<KeyEvent>(Intent.EXTRA_KEY_EVENT)
                    if (event?.action == KeyEvent.ACTION_DOWN) {
                        val kind = when (event.keyCode) {
                            KeyEvent.KEYCODE_MEDIA_NEXT -> "next"
                            KeyEvent.KEYCODE_MEDIA_PREVIOUS -> "previous"
                            else -> null
                        }
                        if (kind != null) {
                            nativeEvent(JSONObject().put("type", kind).toString())
                            return true
                        }
                    }
                    return false
                }
            }).build()
        addSession(session)
        handler.post(positionTick)
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession = session

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val result = super.onStartCommand(intent, flags, startId)
        val action = intent?.getStringExtra("action") ?: return result
        val payload = JSONObject(intent.getStringExtra("payload") ?: "{}")
        when (action) {
            "play" -> {
                val uri = payload.optString("uri")
                val metadata = MediaMetadata.Builder()
                    .setTitle(payload.optString("title"))
                    .setArtist(payload.optString("artist"))
                    .build()
                player.setMediaItem(MediaItem.Builder().setUri(uri)
                    .setMediaId(payload.optString("media_id"))
                    .setMediaMetadata(metadata).build())
                player.seekTo(payload.optLong("position", 0).coerceAtLeast(0))
                player.prepare()
                player.play()
            }
            "resume" -> player.play()
            "pause" -> player.pause()
            "seek" -> player.seekTo(payload.optLong("position", 0).coerceAtLeast(0))
            "stop" -> { player.stop(); player.clearMediaItems(); stopSelf() }
        }
        return START_STICKY
    }

    override fun onDestroy() {
        handler.removeCallbacks(positionTick)
        session.release()
        player.release()
        super.onDestroy()
    }

    private inner class TuneDataSource : BaseDataSource(false) {
        private var uri: Uri? = null
        private var position = 0L
        private var remaining = C.LENGTH_UNSET.toLong()
        private var file: FileDataSource? = null

        override fun open(dataSpec: DataSpec): Long {
            uri = dataSpec.uri
            position = dataSpec.position
            remaining = dataSpec.length
            transferInitializing(dataSpec)
            if (dataSpec.uri.scheme == "file") {
                file = FileDataSource()
                val length = file!!.open(dataSpec)
                transferStarted(dataSpec)
                return length
            }
            if (dataSpec.uri.scheme != "tunestash") throw IOException("Invalid audio source")
            transferStarted(dataSpec)
            return remaining
        }

        override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
            if (length == 0) return 0
            file?.let { return it.read(buffer, offset, length) }
            if (remaining == 0L) return C.RESULT_END_OF_INPUT
            val current = uri ?: throw IOException("Audio source closed")
            val parts = current.pathSegments
            if (parts.size != 2) throw IOException("Invalid audio identity")
            val account = current.host?.toLongOrNull() ?: throw IOException("Invalid account")
            val chat = parts[0].toLongOrNull() ?: throw IOException("Invalid chat")
            val message = parts[1].toLongOrNull() ?: throw IOException("Invalid message")
            val requested = if (remaining == C.LENGTH_UNSET.toLong()) length else minOf(length.toLong(), remaining).toInt()
            val bytes = nativeReadRange(account, chat, message, position, requested.toLong())
                ?: throw IOException("Telegram audio is unavailable")
            if (bytes.isEmpty()) return C.RESULT_END_OF_INPUT
            bytes.copyInto(buffer, offset, 0, bytes.size)
            position += bytes.size
            if (remaining != C.LENGTH_UNSET.toLong()) remaining -= bytes.size
            bytesTransferred(bytes.size)
            return bytes.size
        }

        override fun getUri(): Uri? = uri
        override fun close() {
            file?.close()
            file = null
            uri = null
            transferEnded()
        }
    }
}
