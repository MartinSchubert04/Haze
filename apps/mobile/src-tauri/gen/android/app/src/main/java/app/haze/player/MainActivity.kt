package app.haze.player

import android.graphics.Color
import android.os.Bundle
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  private var webView: WebView? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    // La interfaz es siempre oscura: iconos claros en las barras del sistema.
    val bars = SystemBarStyle.dark(Color.TRANSPARENT)
    enableEdgeToEdge(bars, bars)
    super.onCreate(savedInstanceState)

    // "Atras" lo decide la app (cierra lo de arriba o vuelve a la pestaña anterior) y, si no
    // hay a donde volver, Haze se va al fondo. Lo de Tauri (onBackButtonPress) queda atado a
    // la primera Activity: el proceso sigue vivo con la musica y al volver a abrir Haze hay
    // una Activity nueva sin nadie que atienda "atras", que entonces cerraba la app. Este se
    // registra en cada una.
    onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        val view = webView ?: return goHome()
        view.evaluateJavascript("window.hazeBack ? window.hazeBack() : false") { handled ->
          if (handled != "true") goHome()
        }
      }
    })
  }

  override fun onWebViewCreate(webView: WebView) {
    this.webView = webView
  }

  private fun goHome() {
    moveTaskToBack(true)
  }
}
