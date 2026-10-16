package app.haze.player.plugin

import android.Manifest
import android.app.Activity
import android.net.Uri
import android.os.Build
import android.util.Base64
import app.tauri.Logger
import app.tauri.PermissionState
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.util.concurrent.Executors

/** `Manifest.permission.ACCESS_LOCAL_NETWORK`, que aparece recien en el SDK 37. */
private const val LOCAL_NETWORK = "android.permission.ACCESS_LOCAL_NETWORK"

@InvokeArg
class ListenArgs {
  lateinit var channel: Channel
}

@InvokeArg
class LoadArgs {
  lateinit var uri: String
  var start: Double = 0.0
  var paused: Boolean = false
  var title: String? = null
  var artist: String? = null
  var album: String? = null
  var artwork: String? = null
  /** Es un link (YouTube...): hay que sacar la URL del audio con yt-dlp. */
  var stream: Boolean = false
}

@InvokeArg
class UpdateArgs {
  lateinit var url: String
  lateinit var sha256: String
}

@InvokeArg
class YtRunArgs {
  lateinit var args: List<String>
}

@InvokeArg
class YtDownloadArgs {
  var id: Long = 0
  lateinit var args: List<String>
}

@InvokeArg
class PausedArgs {
  var paused: Boolean = false
}

@InvokeArg
class SeekArgs {
  var seconds: Double = 0.0
}

@InvokeArg
class VolumeArgs {
  var volume: Double = 100.0
}

@InvokeArg
class CoverArgs {
  lateinit var uri: String
  var size: Int = 512
}

/**
 * Lo que Rust le pide a Android. Solo lo llama Rust (`run_mobile_plugin`), nunca la
 * interfaz directo. Los comandos llegan en el hilo principal.
 */
@TauriPlugin(
  permissions = [
    Permission(strings = [Manifest.permission.READ_MEDIA_AUDIO], alias = "audio"),
    Permission(strings = [Manifest.permission.READ_EXTERNAL_STORAGE], alias = "storage"),
    Permission(strings = [LOCAL_NETWORK], alias = "network"),
  ]
)
class HazePlayerPlugin(private val activity: Activity) : Plugin(activity) {
  private val context get() = activity.applicationContext

  /** Leer MediaStore o una tapa puede tardar: fuera del hilo principal. */
  private val io = Executors.newSingleThreadExecutor()

  /** yt-dlp: una busqueda no espera a que termine una descarga. */
  private val yt = Executors.newCachedThreadPool()

  /** Android 13 separo el permiso de audio del de almacenamiento. */
  private val accessAlias get() = if (Build.VERSION.SDK_INT >= 33) "audio" else "storage"

  @Command
  fun listen(invoke: Invoke) {
    Playback.channel = invoke.parseArgs(ListenArgs::class.java).channel
    Playback.connect(context)
    invoke.resolve()
  }

  @Command
  fun load(invoke: Invoke) {
    Playback.load(context, invoke.parseArgs(LoadArgs::class.java))
    invoke.resolve()
  }

  @Command
  fun setPaused(invoke: Invoke) {
    Playback.setPaused(context, invoke.parseArgs(PausedArgs::class.java).paused)
    invoke.resolve()
  }

  @Command
  fun seek(invoke: Invoke) {
    Playback.seek(context, invoke.parseArgs(SeekArgs::class.java).seconds)
    invoke.resolve()
  }

  @Command
  fun setVolume(invoke: Invoke) {
    Playback.setVolume(context, invoke.parseArgs(VolumeArgs::class.java).volume)
    invoke.resolve()
  }

  @Command
  fun stop(invoke: Invoke) {
    Playback.stop(context)
    invoke.resolve()
  }

  @Command
  fun checkAccess(invoke: Invoke) {
    resolveAccess(invoke)
  }

  @Command
  fun requestAccess(invoke: Invoke) {
    if (getPermissionState(accessAlias) == PermissionState.GRANTED) {
      resolveAccess(invoke)
    } else {
      requestPermissionForAlias(accessAlias, invoke, "accessResult")
    }
  }

  @PermissionCallback
  fun accessResult(invoke: Invoke) {
    resolveAccess(invoke)
  }

  private fun resolveAccess(invoke: Invoke) {
    val result = JSObject()
    result.put("granted", getPermissionState(accessAlias) == PermissionState.GRANTED)
    invoke.resolve(result)
  }

  /**
   * Android 17 corta las conexiones a la red local (el broker de Home Assistant) si no
   * esta este permiso. Antes no existia: se da por concedido.
   */
  private fun networkGranted() = Build.VERSION.SDK_INT < 37 || getPermissionState("network") == PermissionState.GRANTED

  @Command
  fun checkNetwork(invoke: Invoke) {
    resolveNetwork(invoke)
  }

  @Command
  fun requestNetwork(invoke: Invoke) {
    if (networkGranted()) {
      resolveNetwork(invoke)
    } else {
      requestPermissionForAlias("network", invoke, "networkResult")
    }
  }

  @PermissionCallback
  fun networkResult(invoke: Invoke) {
    resolveNetwork(invoke)
  }

  private fun resolveNetwork(invoke: Invoke) {
    val result = JSObject()
    result.put("granted", networkGranted())
    invoke.resolve(result)
  }

  /** Corre yt-dlp y devuelve lo que imprimio (el JSON de una busqueda o un link). */
  @Command
  fun ytRun(invoke: Invoke) {
    val args = invoke.parseArgs(YtRunArgs::class.java).args
    yt.execute {
      try {
        val result = JSObject()
        result.put("out", Youtube.run(context, args))
        invoke.resolve(result)
      } catch (e: Throwable) {
        invoke.reject(e.message ?: "yt-dlp fallo")
      }
    }
  }

  /** Baja y guarda en Music/Haze. Cada linea de yt-dlp va por el canal como "download". */
  @Command
  fun ytDownload(invoke: Invoke) {
    val a = invoke.parseArgs(YtDownloadArgs::class.java)
    yt.execute {
      try {
        val saved = Youtube.download(context, a.id, a.args) { line -> Playback.download(a.id, line) }
        val result = JSObject()
        result.put("saved", saved)
        invoke.resolve(result)
      } catch (e: Throwable) {
        invoke.reject(e.message ?: "yt-dlp fallo")
      }
    }
  }

  /**
   * "Atras" en la pantalla de inicio: la app se va al fondo en vez de cerrarse, como las
   * de musica. Sigue sonando y al volver esta todo como estaba.
   */
  @Command
  fun minimize(invoke: Invoke) {
    activity.moveTaskToBack(true)
    invoke.resolve()
  }

  // ------------------------------------------------------------------ actualizaciones

  /** El ultimo Release de Android en GitHub: {version, url, sha256} o {} si no hay. */
  @Command
  fun updateLatest(invoke: Invoke) {
    yt.execute {
      try {
        val latest = Updater.latest()
        invoke.resolve(if (latest == null) JSObject() else JSObject(latest.toString()))
      } catch (e: Throwable) {
        invoke.reject(e.message ?: "No se pudo consultar GitHub")
      }
    }
  }

  /** Baja y verifica el APK. El avance va por el canal como "update". */
  @Command
  fun updateDownload(invoke: Invoke) {
    val a = invoke.parseArgs(UpdateArgs::class.java)
    yt.execute {
      try {
        Updater.download(context, a.url, a.sha256) { p -> Playback.update(p) }
        invoke.resolve()
      } catch (e: Throwable) {
        invoke.reject(e.message ?: "No se pudo bajar la actualizacion")
      }
    }
  }

  /**
   * Instala lo bajado. Sin el permiso "Instalar apps desconocidas" abre ese ajuste y
   * responde `needsPermission`: al volver, se toca "Actualizar" de nuevo.
   */
  @Command
  fun updateInstall(invoke: Invoke) {
    val result = JSObject()
    if (!Updater.allowed(context)) {
      Updater.openPermission(activity)
      result.put("needsPermission", true)
      invoke.resolve(result)
      return
    }
    yt.execute {
      try {
        Updater.install(context)
        result.put("needsPermission", false)
        invoke.resolve(result)
      } catch (e: Throwable) {
        invoke.reject(e.message ?: "No se pudo instalar")
      }
    }
  }

  /**
   * Al salir de la app sin musica sonando, se instala lo que haya listo, si Android deja
   * hacerlo sin preguntar. Si no, queda para "Actualizar ahora" en Ajustes.
   */
  override fun onStop() {
    if (Updater.ready != null && !Playback.playing && Updater.silent(context)) {
      yt.execute {
        try {
          Updater.install(context)
        } catch (e: Throwable) {
          Logger.error("haze-player: no se pudo instalar la actualizacion: ${e.message}")
        }
      }
    }
  }

  @Command
  fun library(invoke: Invoke) {
    io.execute {
      try {
        val result = JSObject()
        result.put("songs", Library.query(context))
        invoke.resolve(result)
      } catch (e: Throwable) {
        invoke.reject(e.message ?: e.toString())
      }
    }
  }

  @Command
  fun cover(invoke: Invoke) {
    val args = invoke.parseArgs(CoverArgs::class.java)
    io.execute {
      val result = JSObject()
      val bitmap = Covers.load(context, Uri.parse(args.uri), args.size)
      result.put("data", bitmap?.let { Base64.encodeToString(Covers.jpeg(it), Base64.NO_WRAP) })
      invoke.resolve(result)
    }
  }
}
