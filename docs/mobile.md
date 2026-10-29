# Haze para Android

Plan para la versión de celular: dónde va a vivir, qué se reusa de escritorio y en qué
orden se hace. Las cinco fases (núcleo compartido, reproductor local, Home Assistant, YouTube y
actualizaciones) están hechas. Falta crear la keystore de firma (ver "Publicar") y
probar las actualizaciones en un celular real.

Solo Android. En iOS no se puede instalar por fuera de la App Store, la App Store no
acepta una app que baja de YouTube y yt-dlp no corre ahí.

## Dónde va cada cosa

```
apps/
  desktop/          la app de Windows (Tauri + Svelte), como hoy
  mobile/           la app de Android (Tauri 2 mobile + Svelte)
    src/            interfaz: pestañas abajo, mini reproductor, hojas
    src-tauri/      shell de Tauri para Android
      gen/android/  proyecto de Gradle que genera `tauri android init`
crates/
  haze-core/        lo que comparten las dos: cola, ajustes, reproductor, MQTT y YouTube
plugins/
  haze-player/      plugin de Tauri en Kotlin: Media3/ExoPlayer + servicio en primer plano
packages/
  ui/               componentes y store de Svelte que usan las dos apps
assets/
  icon.svg          fuente del ícono, de acá salen los de escritorio y Android
```

`packages/ui` por ahora tiene lo que de verdad comparten las dos interfaces: los tipos que
devuelve Rust, el formato de tiempos, los colores de acento y el tema (con la fuente). Los
componentes no se comparten: el armado de cada pantalla es distinto y copiarlos salió más
simple que parametrizarlos.

## Qué se reusa

| Pieza de escritorio | En Android |
|---|---|
| `queue.rs`, `settings.rs` | Pasan a `haze-core` sin cambios. |
| `player.rs` | La lógica (contexto, encolados, aleatorio, repetir) va a `haze-core` detrás de un trait `Engine`. Escritorio lo implementa con mpv y Android con el plugin. |
| `library.rs` | La lectura de tags con lofty se reusa. Recorrer carpetas no: Android lista la música con MediaStore. |
| `mqtt.rs` | Está en `haze-core` (feature `mqtt`) y lo usan las dos. |
| Svelte (vistas, store, componentes) | Se reusan las vistas y el store; cambia el armado de la pantalla. |
| `mpv.rs`, `tools.rs` | No. mpv como proceso con un named pipe no existe en Android. |
| `youtube.rs` | Leer el JSON de yt-dlp y los argumentos de descarga están en `haze-core`. Escritorio corre yt-dlp como programa; Android, con `youtubedl-android`. |
| Bandeja, atajos globales, autostart, single-instance, mini ventana, vidrio de Windows | Se sacan. |
| `media_keys.rs` | Lo reemplaza la sesión de medios de Media3: notificación, pantalla de bloqueo, auriculares. |
| `updater.rs` | Updater propio con `PackageInstaller` (ver abajo). |

## Interfaz

- La barra lateral pasa a pestañas abajo: Canciones, Carpetas, YouTube, Cola.
- La barra del reproductor queda como mini reproductor encima de las pestañas. Al tocarla
  se abre "Reproduciendo" a pantalla completa, que se cierra deslizando hacia abajo.
- El clic derecho pasa a mantener apretado y abrir una hoja con las opciones.
- Ajustes conserva acento, fondo y Home Assistant. Se van las opciones de ventana.

## Fases

**0. Núcleo compartido. Hecho.** `crates/haze-core` tiene `song` (`Song` e `id_for`),
`queue`, `settings` y `player`. El reproductor no sabe de Tauri ni de mpv: pide un
`Engine` (cargar, pausa, seek, volumen, stop), recibe lo que pasa como `EngineEvent` y
avisa los cambios a un `Listener`. En escritorio, `mpv.rs` implementa `Engine` y traduce
el IPC de mpv a `EngineEvent`, y `player.rs` es el `Listener` que emite a las ventanas.
En Android, el plugin `haze-player` va a ser otro `Engine`.

**1. Reproductor local. Hecho.** `apps/mobile` es la app de Tauri para Android y
`plugins/haze-player` es el plugin: un `MediaSessionService` con ExoPlayer (notificación,
pantalla de bloqueo, auriculares y reproducción con la pantalla apagada), la biblioteca por
MediaStore, las tapas y el permiso de audio. Del lado de Rust, `AndroidEngine` implementa
`Engine` y el mismo `Player` de `haze-core` maneja la cola; anterior y siguiente desde la
notificación vuelven a Rust como `PlayerEvent::Previous`/`Next`. La interfaz tiene pestañas
Canciones, Carpetas y Cola, mini reproductor, "Reproduciendo" a pantalla completa, hoja al
mantener apretado y el botón atrás del sistema cierra lo que esté abierto.

Quedó pendiente:

- Las carpetas de música por defecto de `settings` siguen siendo las de Windows. En Android
  quedan vacías y no se usan: la biblioteca sale de MediaStore.
- Si Android destruye la actividad mientras el servicio sigue sonando, los eventos del
  plugin llegan a un canal sin interfaz. La cola sigue en Rust, pero hay que probarlo en
  un celular real con poca memoria.
- En Windows, `tauri android build` necesita el modo de desarrollador activado (crea
  enlaces simbólicos). Sin eso falla al final, después de compilar Rust; el CI corre en
  Linux y no tiene el problema.

**2. Home Assistant. Hecho.** `mqtt.rs` pasó a `haze-core` detrás de un trait `Hooks`:
cada app dice cómo obedecer las órdenes, de dónde salen las carpetas y las tapas, y dónde
mostrar si está conectado. Las entidades son las mismas que en escritorio; el nombre por
defecto del dispositivo es "Haze" más el modelo del teléfono. El puente vive lo que vive
el proceso: mientras suena música lo sostiene el servicio en primer plano y, si Android
mata la app, el broker marca a Haze como desconectado (last will).

Android 17 corta las conexiones a la red local si la app no tiene `ACCESS_LOCAL_NETWORK`
(el sistema lo muestra como "dispositivos cercanos"). Haze lo pide al guardar la
configuración de HA; sin él, Ajustes muestra "Falta permiso" en vez de un timeout.

**3. YouTube. Hecho.** Pestaña YouTube: buscar, escuchar sin bajar, pegar un link (video o
playlist) y bajar. yt-dlp corre con `youtubedl-android` (la librería de Seal), que trae
Python, ffmpeg y QuickJS; Haze actualiza yt-dlp la primera vez y después cada tres días,
porque el que viene adentro queda viejo enseguida. Para escuchar, Kotlin le pide a yt-dlp
la URL del audio y se la pasa a ExoPlayer. Lo bajado se guarda en `Music/Haze` por
MediaStore, así aparece en la biblioteca y en otras apps de música. El APK crece unos 50 MB
por arquitectura.

Las descargas siguen con la app en el fondo, con la pantalla apagada o sacada de recientes.
Mientras hay algo en la cola corre `DownloadService`, un servicio en primer plano
(`dataSync`) con el avance en una notificación. Al terminar, si la app no está a la vista,
deja un aviso con cuántas se bajaron. Rust maneja la cola y llama a Kotlin por JNI
(`DownloadBridge`), no por el plugin, porque los comandos del plugin necesitan la Activity.
El permiso de notificaciones (Android 13+) se pide con la primera descarga; sin él la
descarga sigue igual, pero la notificación no se ve.

El ffmpeg de `youtubedl-android` 0.18.1 está alineado a páginas de 4 KB y no carga en
celulares con páginas de 16 KB (algunos con Android 15 o más nuevo, y el emulador de 16 KB).
Ahí Haze se da cuenta en la primera descarga y baja el audio tal cual lo da YouTube (m4a),
sin tags ni tapa: el nombre queda "Artista - Tema" y la biblioteca separa el artista del
nombre. Cuando la librería publique un ffmpeg de 16 KB, alcanza con subir la versión.

**4. Actualizaciones. Hecho.** Ver las dos secciones siguientes.

## Actualizaciones

Cada seis horas (y con "Buscar" en Ajustes) Haze se fija si el último Release de GitHub
trae una versión más nueva en `latest-android.json`, la baja de fondo y verifica el sha256.
La firma la verifica Android: un APK firmado con otra clave no se instala encima.

Se instala solo al salir de la app sin música sonando, si Android deja hacerlo sin
preguntar: Android 12 o más nuevo, con el permiso "Instalar apps desconocidas" y Haze como
quien instaló la versión actual (`setRequireUserAction(USER_ACTION_NOT_REQUIRED)`). Si no,
queda "Actualizar ahora" en Ajustes, que la primera vez abre el ajuste del permiso y
después muestra el diálogo del sistema. La sesión queda guardada: al abrir la versión
nueva, vuelve a la misma canción en el mismo segundo.

Probado en el emulador: buscar sin versión nueva publicada y el APK de release firmado. La instalación en sí hay que confirmarla en un celular real con el primer Release.

## Publicar

Escritorio y Android salen juntos, con la misma versión:

```
git tag v0.2.0
git push origin v0.2.0
```

El tag dispara los dos workflows. `Escritorio` sube el instalador y `latest.json`;
`Android` compila el APK de release para arm64, lo firma y sube el APK y
`latest-android.json`. Los dos usan `.github/scripts/release.sh`: el que termina primero
crea el Release (marcado como último) y el otro le suma sus archivos.

La firma sale de cuatro secrets del repo (Settings > Secrets and variables > Actions). Para
crearlos, una sola vez:

```
keytool -genkeypair -keystore haze.jks -alias haze -keyalg RSA -keysize 4096 -validity 36500
base64 -w0 haze.jks   # en Windows: certutil -encode haze.jks haze.b64 (sin las lineas BEGIN/END)
```

| Secret | Valor |
|---|---|
| `ANDROID_KEYSTORE` | `haze.jks` en base64 |
| `ANDROID_KEYSTORE_PASSWORD` | la contraseña de la keystore |
| `ANDROID_KEY_ALIAS` | `haze` |
| `ANDROID_KEY_PASSWORD` | la de la clave (si es la misma, se puede omitir) |

Todas las versiones se firman con la misma keystore. Si se pierde, nadie puede actualizar
sin desinstalar: guardala en dos lugares, fuera del repo.

Para un APK firmado en la PC, crear `apps/mobile/src-tauri/gen/android/keystore.properties`
(está en el `.gitignore`) con `storeFile`, `storePassword`, `keyAlias` y `keyPassword`.
