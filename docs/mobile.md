# Haze para Android

Plan para la versión de celular: dónde va a vivir, qué se reusa de escritorio y en qué
orden se hace. Las fases 0 (núcleo compartido), 1 (reproductor local) y 2 (Home Assistant) ya están; lo
siguiente es la fase 3.

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
  haze-core/        lo que comparten las dos: cola, ajustes, lógica del reproductor y MQTT
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
| `youtube.rs` | Con otro motor: `youtubedl-android` (la librería de Seal), que trae Python y yt-dlp. |
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

- La pestaña YouTube llega con la fase 3.
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

**3. YouTube.** Buscar, escuchar y bajar con `youtubedl-android`. Es lo más frágil y lo que
más agranda el APK (unos 30 a 60 MB).

**4. Actualizaciones.** Ver la sección siguiente.

## Actualizaciones

Igual que en escritorio: busca en GitHub cada tantas horas, baja de fondo, verifica la
firma, instala cuando no está sonando nada y vuelve a la canción en el mismo segundo
(`MY_PACKAGE_REPLACED` reemplaza a `relaunch.json`).

Lo que cambia es que Android pide permiso:

1. Una sola vez, el permiso "Instalar apps desconocidas" para Haze.
2. La primera actualización muestra el diálogo del sistema.
3. Después, en Android 12 o más nuevo, `setRequireUserAction(USER_ACTION_NOT_REQUIRED)`
   permite instalar sin preguntar, porque Haze pasa a ser quien instaló la versión
   actual. Si alguien reinstala el APK a mano, la siguiente vuelve a preguntar. En
   Android 11 o anterior siempre pregunta. Hay que confirmarlo en un dispositivo real.

Todas las versiones se firman con la misma keystore. Si se pierde, nadie puede
actualizar sin desinstalar. Va como secret en GitHub, igual que la clave del updater de
escritorio.

## Publicación

- Tags separados por app: `v*` sigue siendo escritorio; Android usa `mobile-v*`.
- El APK se sube al mismo Release de GitHub con su propio `latest-android.json`.
- Workflow aparte en el CI (`android.yml`), en `ubuntu-latest` con el SDK y el NDK de
  Android. Hoy compila un APK de debug en cada push que toque algo de Android.

## Antes de arrancar

- Android Studio con el SDK, el NDK y un emulador o un celular con depuración USB.
- Rust con los targets `aarch64-linux-android` y `armv7-linux-androideabi`.
- Generar la keystore de firma y guardarla en dos lugares.
