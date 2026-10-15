package app.haze.player.plugin

import android.content.ComponentName
import android.content.Context
import android.net.Uri
import android.os.Handler
import android.os.Looper
import androidx.core.content.ContextCompat
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import app.tauri.Logger
import app.tauri.plugin.Channel
import app.tauri.plugin.JSObject
import com.google.common.util.concurrent.ListenableFuture
import java.util.concurrent.Executors

/**
 * El lado de la app: un MediaController conectado a [PlaybackService]. Vive mientras
 * viva el proceso, no la Activity, porque la musica sigue con la app cerrada.
 *
 * Todo corre en el hilo principal: ahi llegan los comandos del plugin y los eventos de
 * Media3.
 */
internal object Playback {
  /** Por donde se le avisa a Rust lo que pasa. Lo pone el comando `listen`. */
  var channel: Channel? = null

  private var controller: MediaController? = null
  private var connecting: ListenableFuture<MediaController>? = null
  private val pending = ArrayDeque<(MediaController) -> Unit>()
  private val main = Handler(Looper.getMainLooper())

  /** Ya se aviso "loaded" del tema actual: READY vuelve a llegar despues de cada buffer. */
  private var loadedSent = false

  /** Sube con cada carga: si llega otra mientras yt-dlp busca la URL, la vieja se descarta. */
  private var loadSeq = 0

  private val resolver = Executors.newSingleThreadExecutor()

  fun connect(context: Context) {
    if (controller != null || connecting != null) return
    val app = context.applicationContext
    val token = SessionToken(app, ComponentName(app, PlaybackService::class.java))
    val future = MediaController.Builder(app, token)
      .setListener(object : MediaController.Listener {
        // El servicio se cerro (por ejemplo, se saco la app de recientes en pausa): el
        // proximo comando vuelve a conectar y lo levanta.
        override fun onDisconnected(controller: MediaController) {
          this@Playback.controller = null
          stopTicker()
        }
      })
      .buildAsync()
    connecting = future
    future.addListener({
      connecting = null
      val c = try {
        future.get()
      } catch (e: Exception) {
        Logger.error("haze-player: no se pudo conectar con el servicio: ${e.message}")
        pending.clear()
        return@addListener
      }
      controller = c
      c.addListener(listener)
      while (pending.isNotEmpty()) pending.removeFirst()(c)
    }, ContextCompat.getMainExecutor(app))
  }

  /** Corre [block] con el controller, conectando primero si hace falta. */
  private fun run(context: Context, block: (MediaController) -> Unit) {
    val c = controller
    if (c != null) {
      block(c)
    } else {
      pending.addLast(block)
      connect(context)
    }
  }

  fun load(context: Context, args: LoadArgs) {
    val seq = ++loadSeq
    if (!args.stream) return play(context, args, args.uri)
    // yt-dlp tarda unos segundos en dar la URL del audio: mientras, "cargando".
    emit("buffering", true)
    resolver.execute {
      val url = try {
        Youtube.streamUrl(context, args.uri)
      } catch (e: Exception) {
        main.post { if (seq == loadSeq) emit("failed", e.message ?: "No se pudo abrir el link") }
        return@execute
      }
      main.post { if (seq == loadSeq) play(context, args, url) }
    }
  }

  private fun play(context: Context, args: LoadArgs, url: String) = run(context) { c ->
    loadedSent = false
    val meta = MediaMetadata.Builder()
      .setTitle(args.title)
      .setArtist(args.artist)
      .setAlbumTitle(args.album)
      .apply { args.artwork?.let { setArtworkUri(Uri.parse(it)) } }
      .build()
    val item = MediaItem.Builder()
      .setUri(url)
      .setMediaId(args.uri)
      .setMediaMetadata(meta)
      .build()
    c.setMediaItem(item, (args.start * 1000).toLong())
    c.playWhenReady = !args.paused
    c.prepare()
  }

  fun setPaused(context: Context, paused: Boolean) = run(context) { it.playWhenReady = !paused }

  fun seek(context: Context, seconds: Double) = run(context) { it.seekTo((seconds * 1000).toLong()) }

  fun setVolume(context: Context, volume: Double) = run(context) { it.volume = (volume / 100).toFloat() }

  fun stop(context: Context) {
    loadSeq++
    run(context) {
      it.stop()
      it.clearMediaItems()
    }
  }

  /** "Anterior" o "siguiente" desde la notificacion, la pantalla de bloqueo o auriculares. */
  fun remote(type: String) = emit(type)

  /** Una linea de yt-dlp mientras baja. Llega desde otro hilo. */
  fun download(id: Long, line: String) = main.post {
    val o = JSObject()
    o.put("id", id)
    o.put("line", line)
    emit("download", o)
  }

  private fun emit(type: String, value: Any? = null) {
    val ch = channel ?: return
    val o = JSObject()
    o.put("type", type)
    if (value != null) o.put("value", value)
    ch.send(o)
  }

  private val listener = object : Player.Listener {
    override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
      emit("paused", !playWhenReady)
    }

    override fun onPlaybackStateChanged(playbackState: Int) {
      val c = controller ?: return
      when (playbackState) {
        Player.STATE_BUFFERING -> emit("buffering", true)
        Player.STATE_READY -> {
          emit("buffering", false)
          if (!loadedSent) {
            loadedSent = true
            emit("loaded")
            if (c.duration != C.TIME_UNSET) emit("duration", c.duration / 1000.0)
          }
        }
        Player.STATE_ENDED -> emit("ended")
      }
    }

    override fun onIsPlayingChanged(isPlaying: Boolean) {
      if (isPlaying) startTicker() else stopTicker()
      emitPosition()
    }

    override fun onPlayerError(error: PlaybackException) {
      emit("failed", error.localizedMessage ?: error.errorCodeName)
    }
  }

  // La posicion no tiene evento: mientras suena se manda dos veces por segundo.
  private val ticker = object : Runnable {
    override fun run() {
      emitPosition()
      main.postDelayed(this, 500)
    }
  }

  private fun startTicker() {
    main.removeCallbacks(ticker)
    main.post(ticker)
  }

  private fun stopTicker() = main.removeCallbacks(ticker)

  private fun emitPosition() {
    val c = controller ?: return
    emit("position", c.currentPosition / 1000.0)
  }
}
