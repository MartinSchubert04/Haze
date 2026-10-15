package app.haze.player.plugin

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.net.Uri
import android.provider.MediaStore
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.Player
import androidx.media3.common.util.BitmapLoader
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSourceBitmapLoader
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import com.google.common.util.concurrent.ListenableFuture
import com.google.common.util.concurrent.MoreExecutors
import java.io.IOException
import java.util.concurrent.Callable
import java.util.concurrent.Executors

/**
 * El servicio en primer plano que hace sonar la musica. Media3 se encarga de la
 * notificacion, la pantalla de bloqueo, los auriculares y de soltar el audio cuando
 * entra una llamada.
 *
 * Suena de a una cancion: la cola la maneja Rust (haze-core). Por eso "anterior" y
 * "siguiente" se muestran siempre y, en vez de moverse dentro de ExoPlayer, se le avisan
 * a Rust por [Playback.remote].
 */
@OptIn(UnstableApi::class)
class PlaybackService : MediaSessionService() {
  private var session: MediaSession? = null

  override fun onCreate() {
    super.onCreate()
    val exo = ExoPlayer.Builder(this)
      .setAudioAttributes(
        AudioAttributes.Builder()
          .setUsage(C.USAGE_MEDIA)
          .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
          .build(),
        true,
      )
      // Se desconectan los auriculares: pausa en vez de seguir por el parlante.
      .setHandleAudioBecomingNoisy(true)
      .setWakeMode(C.WAKE_MODE_LOCAL)
      .build()

    val open = packageManager.getLaunchIntentForPackage(packageName)?.let {
      PendingIntent.getActivity(this, 0, it, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
    }
    session = MediaSession.Builder(this, QueueButtons(exo))
      .setBitmapLoader(CoverLoader(this))
      .apply { open?.let { setSessionActivity(it) } }
      .build()
  }

  override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

  /** Se saco la app de recientes: si no esta sonando nada, no hay por que quedarse. */
  override fun onTaskRemoved(rootIntent: Intent?) {
    val player = session?.player
    if (player == null || !player.playWhenReady || player.mediaItemCount == 0 || player.playbackState == Player.STATE_ENDED) {
      stopSelf()
    }
  }

  override fun onDestroy() {
    session?.run {
      player.release()
      release()
    }
    session = null
    super.onDestroy()
  }
}

/** ExoPlayer con "anterior" y "siguiente" siempre disponibles, que van a la cola de Rust. */
@OptIn(UnstableApi::class)
private class QueueButtons(player: Player) : ForwardingPlayer(player) {
  private val extra = listOf(
    Player.COMMAND_SEEK_TO_NEXT,
    Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
    Player.COMMAND_SEEK_TO_PREVIOUS,
    Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
  )

  override fun getAvailableCommands(): Player.Commands =
    super.getAvailableCommands().buildUpon().addAll(*extra.toIntArray()).build()

  override fun isCommandAvailable(command: Int): Boolean = command in extra || super.isCommandAvailable(command)

  override fun seekToNext() = Playback.remote("next")
  override fun seekToNextMediaItem() = Playback.remote("next")
  override fun seekToPrevious() = Playback.remote("previous")
  override fun seekToPreviousMediaItem() = Playback.remote("previous")
}

/** Las tapas de MediaStore salen del archivo; el resto (URLs) las baja Media3. */
@OptIn(UnstableApi::class)
private class CoverLoader(private val context: Context) : BitmapLoader {
  private val fallback = DataSourceBitmapLoader(context)
  private val executor = MoreExecutors.listeningDecorator(Executors.newSingleThreadExecutor())

  override fun supportsMimeType(mimeType: String): Boolean = fallback.supportsMimeType(mimeType)

  override fun decodeBitmap(data: ByteArray): ListenableFuture<Bitmap> = fallback.decodeBitmap(data)

  override fun loadBitmap(uri: Uri): ListenableFuture<Bitmap> =
    if (uri.scheme == "content" && uri.authority == MediaStore.AUTHORITY) {
      executor.submit(Callable { Covers.load(context, uri, 512) ?: throw IOException("sin tapa") })
    } else {
      fallback.loadBitmap(uri)
    }
}
