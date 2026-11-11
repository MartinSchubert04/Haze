package app.haze.player.plugin

import android.content.ContentValues
import android.content.Context
import android.net.Uri
import android.os.Environment
import android.provider.MediaStore
import androidx.annotation.RequiresApi
import java.io.File

/**
 * Guardar audio en la musica del telefono por MediaStore (Android 10+), asi lo ven Haze y
 * las otras apps sin permiso de escritura. Lo usan YouTube y la sincronizacion con el server.
 */
@RequiresApi(29)
internal object Media {
  /** Los tipos que Android conoce por extension: con otro, MediaStore cambiaba el nombre. */
  private val MIME = mapOf(
    "mp3" to "audio/mpeg",
    "flac" to "audio/flac",
    "m4a" to "audio/mp4",
    "aac" to "audio/aac",
    "ogg" to "audio/ogg",
    "opus" to "audio/ogg",
    "wav" to "audio/x-wav",
    "wma" to "audio/x-ms-wma",
    "aiff" to "audio/x-aiff",
    "aif" to "audio/x-aiff",
    "webm" to "audio/webm",
  )

  fun mime(name: String): String? = MIME[name.substringAfterLast('.', "").lowercase()]

  /** `folder` es relativa a Music: "Haze" o "Haze/Rock". */
  private fun relative(folder: String) = "${Environment.DIRECTORY_MUSIC}/$folder"

  /**
   * Copia `file` a Music/<folder>/<name>. Mientras se copia queda pendiente (no lo ve nadie
   * a medias); si la copia falla, se borra la entrada.
   */
  fun publish(context: Context, file: File, folder: String, name: String = file.name, mime: String? = mime(name)): Uri {
    val resolver = context.contentResolver
    val values = ContentValues().apply {
      put(MediaStore.Audio.Media.DISPLAY_NAME, name)
      // Sin tipo, MediaStore lo saca de la extension.
      if (mime != null) put(MediaStore.Audio.Media.MIME_TYPE, mime)
      put(MediaStore.Audio.Media.RELATIVE_PATH, relative(folder))
      put(MediaStore.Audio.Media.IS_PENDING, 1)
    }
    val uri = resolver.insert(MediaStore.Audio.Media.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY), values)
      ?: throw IllegalStateException("MediaStore no acepto $name")
    try {
      val out = resolver.openOutputStream(uri) ?: throw IllegalStateException("No se pudo escribir $name")
      out.use { o -> file.inputStream().use { it.copyTo(o) } }
      values.clear()
      values.put(MediaStore.Audio.Media.IS_PENDING, 0)
      resolver.update(uri, values, null, null)
    } catch (e: Throwable) {
      resolver.delete(uri, null, null)
      throw e
    }
    return uri
  }

  /**
   * Si ya hay un archivo con ese nombre en Music/<folder>, de cualquier tipo. Se mira en
   * MediaStore y en el disco: el disco solo muestra lo que la app puede leer.
   */
  fun exists(context: Context, folder: String, name: String): Boolean {
    @Suppress("DEPRECATION")
    val music = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_MUSIC)
    if (File(music, "$folder/$name").exists()) return true
    val files = MediaStore.Files.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY)
    context.contentResolver.query(
      files,
      arrayOf(MediaStore.MediaColumns._ID),
      "${MediaStore.MediaColumns.RELATIVE_PATH} = ? AND ${MediaStore.MediaColumns.DISPLAY_NAME} = ?",
      arrayOf("${relative(folder)}/", name),
      null,
    )?.use { return it.count > 0 }
    return false
  }

  /** El nombre con el que quedo: si el que se pidio estaba ocupado, MediaStore le agrega " (1)". */
  fun name(context: Context, uri: Uri): String? =
    context.contentResolver.query(uri, arrayOf(MediaStore.MediaColumns.DISPLAY_NAME), null, null, null)?.use {
      if (it.moveToFirst()) it.getString(0) else null
    }
}
