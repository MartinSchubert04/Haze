package app.haze.player.plugin

import android.content.ContentUris
import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.media.MediaMetadataRetriever
import android.net.Uri
import android.os.Build
import android.provider.MediaStore
import android.util.Size
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import java.io.ByteArrayOutputStream

/** La musica del telefono, como la indexa Android (MediaStore). */
internal object Library {
  fun query(context: Context): JSArray {
    val out = JSArray()
    val collection = MediaStore.Audio.Media.EXTERNAL_CONTENT_URI
    val columns = mutableListOf(
      MediaStore.Audio.Media._ID,
      MediaStore.Audio.Media.TITLE,
      MediaStore.Audio.Media.DISPLAY_NAME,
      MediaStore.Audio.Media.ARTIST,
      MediaStore.Audio.Media.ALBUM,
      MediaStore.Audio.Media.DURATION,
      MediaStore.Audio.Media.DATE_MODIFIED,
      MediaStore.Audio.Media.SIZE,
      @Suppress("DEPRECATION") MediaStore.Audio.Media.DATA,
    )
    if (Build.VERSION.SDK_INT >= 29) columns += MediaStore.Audio.Media.RELATIVE_PATH

    context.contentResolver.query(
      collection,
      columns.toTypedArray(),
      "${MediaStore.Audio.Media.IS_MUSIC} != 0",
      null,
      null,
    )?.use { c ->
      val id = c.getColumnIndexOrThrow(MediaStore.Audio.Media._ID)
      val title = c.getColumnIndexOrThrow(MediaStore.Audio.Media.TITLE)
      val name = c.getColumnIndexOrThrow(MediaStore.Audio.Media.DISPLAY_NAME)
      val artist = c.getColumnIndexOrThrow(MediaStore.Audio.Media.ARTIST)
      val album = c.getColumnIndexOrThrow(MediaStore.Audio.Media.ALBUM)
      val duration = c.getColumnIndexOrThrow(MediaStore.Audio.Media.DURATION)
      val modified = c.getColumnIndexOrThrow(MediaStore.Audio.Media.DATE_MODIFIED)
      val size = c.getColumnIndexOrThrow(MediaStore.Audio.Media.SIZE)
      @Suppress("DEPRECATION") val data = c.getColumnIndex(MediaStore.Audio.Media.DATA)
      val relative = if (Build.VERSION.SDK_INT >= 29) c.getColumnIndex(MediaStore.Audio.Media.RELATIVE_PATH) else -1

      while (c.moveToNext()) {
        val uri = ContentUris.withAppendedId(collection, c.getLong(id))
        val file = c.getString(name).orEmpty()
        val song = JSObject()
        song.put("uri", uri.toString())
        song.put("title", c.getString(title)?.takeIf { it.isNotBlank() } ?: file.substringBeforeLast('.'))
        song.put("artist", known(c.getString(artist)))
        song.put("album", known(c.getString(album)))
        song.put("durationMs", c.getLong(duration))
        song.put("mtime", c.getLong(modified))
        song.put("name", file)
        song.put("size", c.getLong(size))
        song.put("folder", folder(c.getStringOrNull(relative), c.getStringOrNull(data)))
        out.put(song)
      }
    }
    return out
  }

  /** MediaStore pone "<unknown>" cuando el archivo no trae el dato. */
  private fun known(text: String?): String = text?.takeIf { it != MediaStore.UNKNOWN_STRING }.orEmpty()

  /**
   * La carpeta relativa al almacenamiento ("Music/Rock"), igual en todas las versiones:
   * desde Android 10 viene hecha; antes se saca de la ruta del archivo.
   */
  private fun folder(relative: String?, data: String?): String {
    if (!relative.isNullOrEmpty()) return relative.trimEnd('/')
    val parent = data?.substringBeforeLast('/', "") ?: return ""
    return parent.replace(Regex("^/storage/(emulated/\\d+|[^/]+)/?"), "")
  }

  private fun android.database.Cursor.getStringOrNull(index: Int): String? =
    if (index < 0 || isNull(index)) null else getString(index)
}

/** La tapa que trae el archivo adentro. */
internal object Covers {
  fun load(context: Context, uri: Uri, size: Int): Bitmap? = try {
    if (Build.VERSION.SDK_INT >= 29) {
      context.contentResolver.loadThumbnail(uri, Size(size, size), null)
    } else {
      embedded(context, uri, size)
    }
  } catch (e: Exception) {
    null
  }

  fun jpeg(bitmap: Bitmap): ByteArray {
    val out = ByteArrayOutputStream()
    bitmap.compress(Bitmap.CompressFormat.JPEG, 88, out)
    return out.toByteArray()
  }

  private fun embedded(context: Context, uri: Uri, size: Int): Bitmap? {
    val retriever = MediaMetadataRetriever()
    try {
      retriever.setDataSource(context, uri)
      val bytes = retriever.embeddedPicture ?: return null
      val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
      BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
      var sample = 1
      while (bounds.outWidth / (sample * 2) >= size && bounds.outHeight / (sample * 2) >= size) sample *= 2
      return BitmapFactory.decodeByteArray(bytes, 0, bytes.size, BitmapFactory.Options().apply { inSampleSize = sample })
    } finally {
      retriever.release()
    }
  }
}
