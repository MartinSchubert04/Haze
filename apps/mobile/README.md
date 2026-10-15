# Haze para Android

El plan, qué se reusa de escritorio y en qué fase va cada cosa está en
[docs/mobile.md](../../docs/mobile.md).

## Cómo está hecho

| Parte | Qué hace |
|---|---|
| `src-tauri/` (Rust) | Biblioteca, cola, reproductor y Home Assistant (los de `crates/haze-core`), comandos para la interfaz. |
| `plugins/haze-player/` (Kotlin + Rust) | Media3/ExoPlayer en un servicio en primer plano, la biblioteca de MediaStore, las tapas y los permisos de audio y de red local. |
| `src/` (Svelte 5) | La interfaz: pestañas abajo, mini reproductor, "Reproduciendo" a pantalla completa. Tipos, formato y tema salen de `packages/ui`. |
| `src-tauri/gen/android/` | El proyecto de Gradle que generó `tauri android init`. |

## Desarrollo

Hace falta Android Studio (SDK y NDK), Java 21 y los targets de Rust:

```
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android
```

La CLI de Tauri lee el SDK y el NDK de estas variables:

```
ANDROID_HOME=%LOCALAPPDATA%\Android\Sdk
NDK_HOME=%ANDROID_HOME%\ndk\28.0.13004108
JAVA_HOME=C:\Program Files\Java\jdk-21
```

Desde la raíz, con un emulador abierto o un celular con depuración USB:

```
npm run android            # con recarga en vivo
npm run android:apk        # APK de debug para celulares (arm64)
```

Para el emulador de la PC (x86_64):

```
cd apps/mobile
npx tauri android build --debug --target x86_64 --apk
adb install -r src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

En Windows, `tauri android build` necesita el modo de desarrollador activado
(Configuración → Sistema → Para programadores), porque crea enlaces simbólicos.

Los registros del lado nativo salen en `adb logcat`, filtrando por `haze` o `Tauri`.
