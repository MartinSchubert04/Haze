package app.haze.player.plugin

import android.content.ContentValues
import android.content.Context
import android.media.MediaScannerConnection
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import app.tauri.Logger
import com.yausername.ffmpeg.FFmpeg
import com.yausername.youtubedl_android.YoutubeDL
import com.yausername.youtubedl_android.YoutubeDLRequest
import java.io.File
import java.util.concurrent.TimeUnit

/**
 * yt-dlp con `youtubedl-android`, que trae Python y ffmpeg. Los argumentos los arma Rust
 * (`haze_core::youtube`); aca solo se corre y se guarda lo bajado donde lo ve MediaStore.
 *
 * Todo bloquea: se llama desde un hilo aparte, nunca desde el principal.
 */
internal object Youtube {
  private var ready = false

  /** Lo que se guarda: MP3 con ffmpeg; sin ffmpeg, el audio tal cual lo da YouTube. */
  private val AUDIO = mapOf("mp3" to "audio/mpeg", "m4a" to "audio/mp4", "opus" to "audio/ogg", "ogg" to "audio/ogg", "webm" to "audio/webm")

  /** YouTube cambia seguido: yt-dlp se actualiza cada tres dias, como en escritorio. */
  private val UPDATE_EVERY = TimeUnit.DAYS.toMillis(3)

  /** La primera vez descomprime Python (unos segundos) y actualiza yt-dlp. */
  @Synchronized
  fun init(context: Context) {
    if (!ready) {
      YoutubeDL.getInstance().init(context)
      FFmpeg.getInstance().init(context)
      ready = true
    }
    val prefs = context.getSharedPreferences("haze-youtube", Context.MODE_PRIVATE)
    val now = System.currentTimeMillis()
    if (now - prefs.getLong("updated", 0) < UPDATE_EVERY) return
    try {
      YoutubeDL.getInstance().updateYoutubeDL(context, YoutubeDL.UpdateChannel.STABLE)
      prefs.edit().putLong("updated", now).apply()
    } catch (e: Exception) {
      // Sin internet o GitHub caido: se sigue con la version que haya.
      Logger.error("haze-player: no se pudo actualizar yt-dlp: ${e.message}")
    }
  }

  /** Corre yt-dlp y devuelve lo que imprimio. Si falla, la excepcion trae el error. */
  fun run(context: Context, args: List<String>, onLine: ((String) -> Unit)? = null): String {
    init(context)
    val request = YoutubeDLRequest(emptyList<String>())
    request.addCommands(args)
    val response = YoutubeDL.getInstance().execute(request, null) { _, _, line -> onLine?.invoke(line) }
    return response.out
  }

  /** La URL directa del audio, para ExoPlayer. Vence en unas horas: se pide en cada carga. */
  fun streamUrl(context: Context, url: String): String =
    run(context, listOf("--ignore-config", "--no-warnings", "--no-playlist", "-f", "bestaudio[ext=m4a]/bestaudio/best", "-g", url))
      .lines()
      .first { it.startsWith("http") }

  /**
   * Baja en una carpeta temporal y pasa cada audio a Music/Haze (con la subcarpeta de la
   * playlist, si hay). Devuelve cuantos archivos guardo.
   */
  fun download(context: Context, id: Long, args: List<String>, onLine: (String) -> Unit): Int {
    val tmp = File(context.cacheDir, "yt/$id")
    tmp.deleteRecursively()
    tmp.mkdirs()
    try {
      // Solo cuenta lo que yt-dlp termino (`--print after_move:FILE ...`): si fallo la
      // conversion, en la carpeta queda el audio original y ese no va.
      val done = mutableListOf<File>()
      val failure = try {
        run(context, listOf("-P", tmp.absolutePath) + args) { line ->
          if (line.startsWith("FILE ")) done.add(File(line.removePrefix("FILE ").trim()))
          onLine(line)
        }
        null
      } catch (e: Exception) {
        e
      }
      // En una playlist algun video puede fallar (privado, bloqueado) sin que se pierda
      // el resto: si algo se bajo, cuenta.
      val files = done.filter { it.isFile && it.extension.lowercase() in AUDIO }
      if (files.isEmpty() && failure != null) throw failure
      for (f in files) publish(context, f, f.parentFile?.takeIf { it != tmp }?.name)
      return files.size
    } finally {
      tmp.deleteRecursively()
    }
  }

  private fun publish(context: Context, file: File, sub: String?) {
    val folder = if (sub == null) "Haze" else "Haze/$sub"
    if (Build.VERSION.SDK_INT >= 29) {
      val resolver = context.contentResolver
      val values = ContentValues().apply {
        put(MediaStore.Audio.Media.DISPLAY_NAME, file.name)
        put(MediaStore.Audio.Media.MIME_TYPE, AUDIO[file.extension.lowercase()])
        put(MediaStore.Audio.Media.RELATIVE_PATH, "${Environment.DIRECTORY_MUSIC}/$folder")
        put(MediaStore.Audio.Media.IS_PENDING, 1)
      }
      val uri = resolver.insert(MediaStore.Audio.Media.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY), values)
        ?: throw IllegalStateException("MediaStore no acepto ${file.name}")
      resolver.openOutputStream(uri).use { out -> file.inputStream().use { it.copyTo(out!!) } }
      values.clear()
      values.put(MediaStore.Audio.Media.IS_PENDING, 0)
      resolver.update(uri, values, null, null)
    } else {
      // Android 9 o anterior: la carpeta de musica propia de la app, que el escaner indexa
      // sin pedir permiso de escritura.
      val dir = File(context.getExternalFilesDir(Environment.DIRECTORY_MUSIC), folder).apply { mkdirs() }
      val dest = File(dir, file.name)
      file.copyTo(dest, overwrite = true)
      MediaScannerConnection.scanFile(context, arrayOf(dest.absolutePath), arrayOf(AUDIO[file.extension.lowercase()]), null)
    }
  }
}
