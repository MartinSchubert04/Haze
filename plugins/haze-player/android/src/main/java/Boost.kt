package app.haze.player.plugin

import android.media.audiofx.LoudnessEnhancer
import app.tauri.Logger
import kotlin.math.roundToInt

/**
 * Refuerzo de volumen (Ajustes → Reproduccion): para auriculares que suenan bajos aun con
 * el telefono al maximo. Es el LoudnessEnhancer de Android sobre la sesion de audio de
 * ExoPlayer, que sube la ganancia con un limitador para no saturar.
 *
 * Los dB los manda Rust al arrancar y al cambiarlos; el efecto lo engancha [PlaybackService]
 * cuando se crea. Todo en el hilo principal.
 */
internal object Boost {
  private var db = 0.0
  private var enhancer: LoudnessEnhancer? = null

  fun set(db: Double) {
    this.db = db.coerceIn(0.0, 12.0)
    apply()
  }

  fun attach(audioSession: Int) {
    release()
    enhancer = try {
      LoudnessEnhancer(audioSession)
    } catch (e: Throwable) {
      Logger.error("haze-player: sin refuerzo de volumen: ${e.message}")
      null
    }
    apply()
  }

  fun release() {
    enhancer?.release()
    enhancer = null
  }

  private fun apply() {
    val e = enhancer ?: return
    try {
      // En milibeles.
      e.setTargetGain((db * 100).roundToInt())
      e.enabled = db > 0
    } catch (t: Throwable) {
      Logger.error("haze-player: no se pudo aplicar el refuerzo: ${t.message}")
    }
  }
}
