package app.haze.player.plugin

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
    try {
      val context = Playback.app ?: throw IllegalStateException("El reproductor todavia no arranco")
      val list = JSONArray(args).let { a -> List(a.length()) { a.getString(it) } }
      out.put("saved", Youtube.download(context, id, list) { line -> Playback.download(id, line) })
    } catch (e: Throwable) {
      out.put("error", e.message ?: "yt-dlp fallo")
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
}
