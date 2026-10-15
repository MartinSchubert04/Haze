# Haze

<img width="1919" height="1130" alt="image" src="https://github.com/user-attachments/assets/8663b5e9-3b7b-46bb-8735-a18497115f2c" />

Reproductor de música para Windows: liviano, queda en la bandeja y sigue sonando con la
ventana cerrada.

- **Biblioteca local:** varias carpetas fuente, búsqueda, orden y un índice A-Z.
- **Cola de verdad:** el contexto (carpeta, todas las canciones) y lo que encolás a mano
  son cosas distintas; lo encolado suena antes y después se retoma el contexto.
- **YouTube:** buscar y escuchar sin bajar, o pegar un link (Ctrl V en cualquier lado, o
  arrastrarlo a la ventana) y bajarlo como MP3 con título, artista y tapa cuadrada.
  Sirven también las playlists y los links de SoundCloud o Bandcamp.
- **Carpetas favoritas:** son los destinos de descarga y los accesos de la barra lateral.
- **Mini reproductor:** una ventanita fija, siempre arriba.
- **Home Assistant por MQTT:** las entidades aparecen solas (estado, canción, tapa, botones,
  volumen, aleatorio, repetir y "reproducir carpeta").
- **Teclas multimedia** y el panel de medios de Windows.
- **Personalización:** acento pastel, fondo de vidrio, bruma, una imagen o un color.

## Cómo está hecho

Es un monorepo. Por ahora tiene una sola app:

| Carpeta | Qué es |
|---|---|
| `apps/desktop/` | La app de Windows. |
| `apps/mobile/` | La futura app de Android. El plan está en [docs/mobile.md](docs/mobile.md). |
| `crates/haze-core/` | Cola, ajustes y reproductor, sin Tauri ni mpv. Lo comparten las dos apps. |
| `assets/` | Fuente del ícono. |

Dentro de `apps/desktop/`:

| Parte | Qué hace |
|---|---|
| `src-tauri/` (Rust, Tauri 2) | Biblioteca, cola, reproductor, YouTube, MQTT, bandeja. Es lo único que queda vivo de fondo. |
| `mpv` | Motor de audio, en un proceso aparte manejado por su IPC. Reproduce cualquier formato y streams. |
| `yt-dlp` + `deno` + `ffmpeg` | Buscar y bajar de YouTube. |
| `src/` (Svelte 5) | La interfaz. Al cerrar la ventana se destruye el WebView entero, por eso de fondo pesa poco. |

Los programas externos no vienen en el instalador: se bajan solos a
`%LOCALAPPDATA%\app.haze.player\bin` la primera vez que hacen falta. yt-dlp se
actualiza solo cada tres días, porque YouTube cambia seguido.

## Desarrollo

Requiere Rust (con las Build Tools de Visual Studio) y Node 24.

Todo se corre desde la raíz:

```
npm install
npm run dev            # la app de escritorio con recarga en vivo
npm run check          # tipos
cargo test
```

Para regenerar los íconos después de cambiar `assets/icon.svg`:

```
cd apps/desktop
npx tauri icon ../../assets/icon.svg
```

Ese comando también genera íconos de Android, iOS y la Store de Windows: de
`src-tauri/icons/` se conservan solo los que lista `tauri.conf.json`.

Para depurar sin pantalla (por ejemplo, desde un servicio):

- `HAZE_DEVTOOLS_PORT=9333` abre el protocolo de DevTools del WebView en ese puerto.
- `HAZE_MPV_ARGS="--ao=null"` le pasa argumentos extra a mpv (sirve para probar sin
  parlantes).

## Publicar una versión

```
git tag v0.2.0
git push origin v0.2.0
```

El workflow `CI` compila el instalador y lo sube a Releases. La app avisa en Ajustes
cuando hay una versión nueva.

## Datos

- Ajustes: `%APPDATA%\app.haze.player\settings.json`. La clave de MQTT queda ahí en
  texto plano.
- Cache de la biblioteca y sesión: `%LOCALAPPDATA%\app.haze.player\`.
