package app.haze.player.plugin

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.os.SystemClock
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import app.tauri.Logger

/**
 * Servicio en primer plano mientras hay descargas de YouTube. Sin el, con la app en el
 * fondo y sin musica sonando, Android congela o mata el proceso y la descarga se corta.
 * Muestra el avance en una notificacion y, al terminar, avisa cuantas se bajaron.
 *
 * Lo maneja Rust por [DownloadBridge]: arranca con la primera descarga (siempre con la
 * app a la vista, que es cuando Android deja arrancarlo) y se va cuando se vacia la cola.
 * Todo corre en el hilo principal.
 */
class DownloadService : Service() {
  private var wake: PowerManager.WakeLock? = null

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onCreate() {
    super.onCreate()
    instance = this
    // Con la pantalla apagada la CPU se duerme y yt-dlp queda trabado.
    wake = (getSystemService(Context.POWER_SERVICE) as PowerManager)
      .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "haze:downloads")
      .apply {
        setReferenceCounted(false)
        acquire(WAKE_TIMEOUT)
      }
  }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    // Arrancado con startForegroundService: hay que pasar a primer plano si o si, aunque
    // la cola ya se haya vaciado mientras tanto.
    try {
      val type = if (Build.VERSION.SDK_INT >= 29) ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC else 0
      ServiceCompat.startForeground(this, PROGRESS_ID, progressNotification(this), type)
    } catch (e: Exception) {
      Logger.error("haze-player: no se pudo mostrar la descarga en primer plano: ${e.message}")
    }
    if (!active) finish()
    // Si Android lo mata, no se recrea solo: Rust ya no tiene la cola.
    return START_NOT_STICKY
  }

  /** Android 15 corta los de sincronizacion despues de 6 horas en el dia. */
  override fun onTimeout(startId: Int, fgsType: Int) {
    finish()
  }

  private fun finish() {
    ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
    stopSelf()
  }

  override fun onDestroy() {
    wake?.takeIf { it.isHeld }?.release()
    wake = null
    if (instance === this) instance = null
    super.onDestroy()
  }

  companion object {
    private const val CHANNEL = "downloads"
    private const val PROGRESS_ID = 2001
    private const val DONE_ID = 2002
    private val WAKE_TIMEOUT = 6 * 60 * 60 * 1000L

    /** Las notificaciones de avance se piden seguido; Android descarta si son demasiadas. */
    private const val MIN_GAP_MS = 700L

    private val main = Handler(Looper.getMainLooper())
    private var instance: DownloadService? = null

    /** Hay descargas en cola o bajandose (lo dice Rust). */
    @Volatile
    var active = false
      private set

    /** La app esta a la vista: el aviso final sobra, ya se ve en la pantalla. */
    @Volatile
    var visible = false

    private var title = ""
    private var progress = 0f
    private var converting = false
    private var detail = ""
    private var lastNotify = 0L

    /** Hay descargas: arranca el servicio (si no estaba) y muestra el avance. */
    fun start(context: Context) = main.post {
      active = true
      if (instance == null) {
        try {
          ContextCompat.startForegroundService(context, Intent(context, DownloadService::class.java))
        } catch (e: Exception) {
          // Android 12+ no deja arrancarlo desde el fondo: la descarga sigue igual, sin la
          // proteccion, mientras el proceso viva.
          Logger.error("haze-player: no se pudo arrancar el servicio de descargas: ${e.message}")
        }
      }
    }

    /** Lo que se esta bajando. [detail] es lo de abajo: "3 de 12 · +2 en cola". */
    fun update(context: Context, title: String, progress: Float, converting: Boolean, detail: String) = main.post {
      // Una linea de yt-dlp que llego despues de que se vacio la cola.
      if (!active) return@post
      val changed = title != this.title || converting != this.converting || detail != this.detail
      this.title = title
      this.progress = progress
      this.converting = converting
      this.detail = detail
      val now = SystemClock.elapsedRealtime()
      if (!changed && now - lastNotify < MIN_GAP_MS) return@post
      lastNotify = now
      if (instance != null) notify(context, PROGRESS_ID, progressNotification(context))
    }

    /** Se vacio la cola: se va el servicio y, con la app en el fondo, avisa como termino. */
    fun stop(context: Context, done: Int, failed: Int, last: String) = main.post {
      active = false
      title = ""
      detail = ""
      progress = 0f
      instance?.finish()
      if (!visible && done + failed > 0) notify(context, DONE_ID, doneNotification(context, done, failed, last))
    }

    private fun channel(context: Context) {
      if (Build.VERSION.SDK_INT < 26) return
      val manager = context.getSystemService(NotificationManager::class.java)
      if (manager.getNotificationChannel(CHANNEL) != null) return
      val ch = NotificationChannel(CHANNEL, "Descargas", NotificationManager.IMPORTANCE_LOW)
      ch.description = "Lo que se baja de YouTube"
      ch.setShowBadge(false)
      manager.createNotificationChannel(ch)
    }

    private fun openApp(context: Context): PendingIntent? =
      context.packageManager.getLaunchIntentForPackage(context.packageName)?.let {
        PendingIntent.getActivity(context, 1, it, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
      }

    private fun progressNotification(context: Context): Notification {
      channel(context)
      val head = when {
        title.isEmpty() -> "Preparando la descarga"
        converting -> "Convirtiendo: $title"
        else -> title
      }
      return NotificationCompat.Builder(context, CHANNEL)
        .setSmallIcon(android.R.drawable.stat_sys_download)
        .setContentTitle(head)
        .apply { if (detail.isNotEmpty()) setContentText(detail) }
        .setProgress(100, (progress * 100).toInt(), title.isEmpty() || converting)
        .setOngoing(true)
        .setOnlyAlertOnce(true)
        .setSilent(true)
        .setCategory(NotificationCompat.CATEGORY_PROGRESS)
        .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
        .setContentIntent(openApp(context))
        .build()
    }

    private fun doneNotification(context: Context, done: Int, failed: Int, last: String): Notification {
      channel(context)
      val head = when {
        done == 1 && failed == 0 -> "Bajada: $last"
        failed == 0 -> "Se bajaron $done descargas"
        done == 0 -> if (failed == 1) "No se pudo bajar $last" else "No se pudieron bajar $failed descargas"
        else -> "Se bajaron $done, fallaron $failed"
      }
      return NotificationCompat.Builder(context, CHANNEL)
        .setSmallIcon(if (done > 0) android.R.drawable.stat_sys_download_done else android.R.drawable.stat_notify_error)
        .setContentTitle(head)
        .apply { if (done > 0) setContentText("Están en Music/Haze") }
        .setAutoCancel(true)
        .setContentIntent(openApp(context))
        .build()
    }

    private fun notify(context: Context, id: Int, n: Notification) {
      if (Build.VERSION.SDK_INT >= 33 &&
        ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
      ) return
      context.getSystemService(NotificationManager::class.java)?.notify(id, n)
    }
  }
}
