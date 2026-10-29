package app.haze.player.plugin

import app.tauri.Logger
import org.json.JSONArray
import org.json.JSONObject

/**
 * Descargas de YouTube que llegan de Rust por JNI, como [EngineBridge]: los comandos del
 * plugin necesitan la Activity, y una descarga tiene que seguir (y la siguiente de la
 * cola, arrancar) con la app en el fondo o sacada de recientes.
 */
object DownloadBridge {
  /**
   * Baja y guarda en Music/Haze. Bloquea hasta que termina: Rust la llama desde un hilo
   * propio. Cada linea de yt-dlp va por el canal como "download". Devuelve
   * `{"saved": n}` o `{"error": "..."}`.
   */
  @JvmStatic
  fun download(id: Long, args: String): String {
    val out = JSONObject()
    // Rust llama desde un hilo propio, enganchado a la JVM sin class loader de contexto, y
    // commons-io (que usa youtubedl-android al actualizar yt-dlp) lo necesita: sin esto
    // tiraba ExceptionInInitializerError y ninguna descarga andaba.
    val thread = Thread.currentThread()
    if (thread.contextClassLoader == null) thread.contextClassLoader = DownloadBridge::class.java.classLoader
    try {
      val context = Playback.app ?: throw IllegalStateException("El reproductor todavia no arranco")
      val list = JSONArray(args).let { a -> List(a.length()) { a.getString(it) } }
      out.put("saved", Youtube.download(context, id, list) { line -> Playback.download(id, line) })
    } catch (e: Throwable) {
      Logger.error("haze-player: la descarga $id fallo: ${e.stackTraceToString()}")
      out.put("error", describe(e))
    }
    return out.toString()
  }

  /**
   * Como va la cola, para el servicio en primer plano y su notificacion:
   * `{"active": true, "title", "progress", "converting", "detail"}` mientras baja y
   * `{"active": false, "done", "failed", "last"}` al vaciarse.
   */
  @JvmStatic
  fun status(json: String) {
    val context = Playback.app ?: return
    val o = JSONObject(json)
    if (!o.optBoolean("active")) {
      DownloadService.stop(context, o.optInt("done"), o.optInt("failed"), o.optString("last"))
    } else if (o.optBoolean("start")) {
      DownloadService.start(context)
    } else {
      DownloadService.update(
        context,
        o.optString("title"),
        o.optDouble("progress", 0.0).toFloat(),
        o.optBoolean("converting"),
        o.optString("detail"),
      )
    }
  }

  /** El mensaje de la excepcion, o su tipo (y el de su causa) si no trae ninguno. */
  internal fun describe(e: Throwable): String {
    e.message?.takeIf { it.isNotBlank() }?.let { return it }
    val cause = e.cause?.let { c -> c.message?.takeIf { it.isNotBlank() } ?: c.javaClass.simpleName }
    return listOfNotNull(e.javaClass.simpleName, cause).joinToString(": ")
  }
}
