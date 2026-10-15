# Haze (escritorio)

Reproductor de música para Windows: liviano, queda en la bandeja y sigue sonando con la
ventana cerrada.

- **Biblioteca local:** varias carpetas fuente, búsqueda, orden y el índice A-Z del celu.
- **Cola como en Android:** el contexto (carpeta, todas las canciones) y lo que encolás a
  mano son cosas distintas. La lógica es un port de `PlayQueue.kt`, con los mismos tests.
- **YouTube:** buscar y escuchar sin bajar, o pegar un link (Ctrl V en cualquier lado, o
  arrastrarlo a la ventana) y bajarlo como MP3 con título, artista y tapa cuadrada.
  Sirven también las playlists y los links de SoundCloud o Bandcamp.
- **Carpetas favoritas:** son los destinos de descarga y los accesos de la barra lateral.
- **Mini reproductor:** una ventanita fija, siempre arriba.
- **Home Assistant por MQTT:** las entidades aparecen solas (estado, canción, tapa, botones,
  volumen, aleatorio, repetir y "reproducir carpeta").
- **Teclas multimedia** y el panel de medios de Windows.
- **Personalización:** acento pastel, fondo de vidrio, el del kiosco, una imagen o un color.

## Cómo está hecho

| Parte | Qué hace |
|---|---|
| `src-tauri/` (Rust, Tauri 2) | Biblioteca, cola, reproductor, YouTube, MQTT, bandeja. Es lo único que queda vivo de fondo. |
| `mpv` | Motor de audio, en un proceso aparte manejado por su IPC. Reproduce cualquier formato y streams. |
| `yt-dlp` + `deno` + `ffmpeg` | Buscar y bajar de YouTube. |
| `src/` (Svelte 5) | La interfaz. Al cerrar la ventana se destruye el WebView entero, por eso de fondo pesa poco. |

Los programas externos no vienen en el instalador: se bajan solos a
`%LOCALAPPDATA%\com.mrtn.haze\bin` la primera vez que hacen falta. yt-dlp se
actualiza solo cada tres días, porque YouTube cambia seguido.

## Desarrollo

Requiere Rust (con las Build Tools de Visual Studio) y Node 24.

```
npm install
npm run tauri dev      # con recarga en vivo
npm run check          # tipos
cd src-tauri && cargo test
```

Para depurar sin pantalla (por ejemplo, desde un servicio):

- `HAZE_DEVTOOLS_PORT=9333` abre el protocolo de DevTools del WebView en ese puerto.
- `HAZE_MPV_ARGS="--ao=null"` le pasa argumentos extra a mpv (sirve para probar sin
  parlantes).

## Publicar una versión

```
git tag desktop-v0.2.0
git push origin desktop-v0.2.0
```

El workflow `Desktop` compila el instalador y lo sube a Releases. La app avisa en Ajustes
cuando hay una versión nueva.

## Datos

- Ajustes: `%APPDATA%\com.mrtn.haze\settings.json`. La clave de MQTT queda ahí en
  texto plano.
- Cache de la biblioteca y sesión: `%LOCALAPPDATA%\com.mrtn.haze\`.
