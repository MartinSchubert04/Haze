package app.haze.player.plugin

import android.Manifest
import android.app.Activity
import android.net.Uri
import android.os.Build
import android.util.Base64
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
  ]
)
class HazePlayerPlugin(private val activity: Activity) : Plugin(activity) {
  private val context get() = activity.applicationContext

  /** Leer MediaStore o una tapa puede tardar: fuera del hilo principal. */
  private val io = Executors.newSingleThreadExecutor()

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

  @Command
  fun library(invoke: Invoke) {
    io.execute {
      try {
        val result = JSObject()
        result.put("songs", Library.query(context))
        invoke.resolve(result)
      } catch (e: Exception) {
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
