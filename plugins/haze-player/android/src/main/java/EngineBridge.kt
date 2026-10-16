package app.haze.player.plugin

import android.os.Handler
import android.os.Looper
import org.json.JSONObject

/**
 * Ordenes del reproductor que llegan de Rust por JNI, sin pasar por el plugin de Tauri.
 * Los comandos del plugin necesitan la Activity; estas no: con la app sacada de recientes
 * la musica sigue y Rust tiene que poder pasar a la siguiente.
 *
 * Rust la llama desde su hilo; todo se hace en el principal, como el resto de [Playback].
 */
object EngineBridge {
  private val main = Handler(Looper.getMainLooper())

  @JvmStatic
  fun call(command: String, json: String) {
    main.post {
      val context = Playback.app ?: return@post
      val o = JSONObject(json)
      when (command) {
        "load" -> Playback.load(context, LoadArgs().apply {
          uri = o.getString("uri")
          start = o.optDouble("start", 0.0)
          paused = o.optBoolean("paused")
          title = o.optStringOrNull("title")
          artist = o.optStringOrNull("artist")
          album = o.optStringOrNull("album")
          artwork = o.optStringOrNull("artwork")
          stream = o.optBoolean("stream")
        })
        "setPaused" -> Playback.setPaused(context, o.optBoolean("paused"))
        "seek" -> Playback.seek(context, o.optDouble("seconds", 0.0))
        "setVolume" -> Playback.setVolume(context, o.optDouble("volume", 100.0))
        "stop" -> Playback.stop(context)
      }
    }
  }

  private fun JSONObject.optStringOrNull(key: String): String? =
    if (isNull(key)) null else optString(key).takeIf { it.isNotEmpty() }
}
