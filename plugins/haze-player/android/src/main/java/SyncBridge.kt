package app.haze.player.plugin

import android.os.Build
import android.os.Environment
import app.tauri.Logger
import org.json.JSONObject
import java.io.File

/**
 * Lo que se baja del server de casa (`haze_core::sync`) llega de Rust por JNI, como
 * [DownloadBridge]: la sincronizacion corre de fondo y no tiene que depender de la Activity.
 */
object SyncBridge {
  /**
   * Pasa `tmp` a Music/Haze/<path> por MediaStore, con su fecha (`mtime`, segundos).
   * Nunca pisa: si ya hay un archivo con ese nombre ahi, no hace nada y avisa el error.
   * Bloquea hasta que termina. Devuelve `{"ok": true}` o `{"error": "..."}`.
   */
  @JvmStatic
  fun save(tmp: String, path: String, mtime: Long): String {
    val out = JSONObject()
    try {
      val context = Playback.app ?: throw IllegalStateException("El reproductor todavia no arranco")
      // Antes de Android 10 no hay RELATIVE_PATH y escribir en Music pide otro permiso.
      if (Build.VERSION.SDK_INT < 29) throw IllegalStateException("Sincronizar necesita Android 10 o mas nuevo")
      val sub = path.substringBeforeLast('/', "")
      val name = path.substringAfterLast('/')
      val folder = if (sub.isEmpty()) "Haze" else "Haze/$sub"
      if (Media.exists(context, folder, name)) throw IllegalStateException("ya existe")
      val uri = Media.publish(context, File(tmp), folder, name)
      if (Media.name(context, uri) != name) {
        // Ocupado entre medio o con otro nombre: seria una copia, no el tema del server.
        context.contentResolver.delete(uri, null, null)
        throw IllegalStateException("ya existe")
      }
      if (mtime > 0) {
        @Suppress("DEPRECATION")
        val music = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_MUSIC)
        File(music, "$folder/$name").setLastModified(mtime * 1000)
      }
      out.put("ok", true)
    } catch (e: Throwable) {
      Logger.error("haze-player: no se pudo guardar $path: ${e.message}")
      out.put("error", DownloadBridge.describe(e))
    }
    return out.toString()
  }
}
