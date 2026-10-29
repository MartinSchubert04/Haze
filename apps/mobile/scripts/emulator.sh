#!/usr/bin/env bash
# Prueba Haze en el emulador de Android desde Git Bash, sin modo desarrollador de Windows.
#
#   bash apps/mobile/scripts/emulator.sh start     arranca el emulador (sin ventana)
#   bash apps/mobile/scripts/emulator.sh install   compila para x86_64, instala y abre Haze
#   bash apps/mobile/scripts/emulator.sh devtools  reenvia las DevTools del WebView a :9222
#
# Con las DevTools reenviadas, scripts/cdp.mjs corre JS en la app, saca capturas y
# muestra la consola (screencap no captura el WebView con el emulador sin ventana):
#
#   node apps/mobile/scripts/cdp.mjs eval "window.__TAURI_INTERNALS__.invoke('bootstrap')"
#   node apps/mobile/scripts/cdp.mjs shot captura.png
#   node apps/mobile/scripts/cdp.mjs logs 10
#
# Los registros nativos: adb logcat | grep -E "haze|Tauri".
set -euo pipefail

SDK="${ANDROID_HOME:-$LOCALAPPDATA/Android/Sdk}"
ADB="$SDK/platform-tools/adb.exe"
AVD="${HAZE_AVD:-Pixel_9_Pro}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
GEN="$ROOT/apps/mobile/src-tauri/gen/android"
export ANDROID_HOME="$SDK"
export NDK_HOME="${NDK_HOME:-$SDK/ndk/28.0.13004108}"
export JAVA_HOME="${JAVA_HOME:-C:/Program Files/Android/Android Studio/jbr}"
export MSYS_NO_PATHCONV=1

case "${1:-}" in
  start)
    # Con 2 GB y render por software la app no llega a responder: mas memoria y GPU.
    "$SDK/emulator/emulator.exe" -avd "$AVD" -skin 1080x2400 -memory 6144 -cores 6 \
      -no-window -no-audio -no-boot-anim -no-snapshot-save -gpu host >/dev/null 2>&1 &
    "$ADB" wait-for-device
    until [ "$("$ADB" shell getprop sys.boot_completed | tr -d '\r')" = "1" ]; do sleep 3; done
    echo "emulador listo"
    ;;
  install)
    rustup target add x86_64-linux-android >/dev/null
    # tauri compila la libreria y despues falla al crear el enlace simbolico a jniLibs
    # (sin modo desarrollador): se copia a mano y Gradle arma el APK sin volver a Rust.
    (cd "$ROOT/apps/mobile" && npx tauri android build --debug --target x86_64 --apk || true)
    mkdir -p "$GEN/app/src/main/jniLibs/x86_64"
    cp "$ROOT/target/x86_64-linux-android/debug/libhaze_mobile_lib.so" "$GEN/app/src/main/jniLibs/x86_64/"
    (cd "$GEN" && ./gradlew.bat assembleX86_64Debug -x rustBuildX86_64Debug)
    git -C "$ROOT" checkout -- apps/mobile/src-tauri/gen/android/gradlew 2>/dev/null || true
    # adb.exe necesita la ruta de Windows (MSYS_NO_PATHCONV la deja como /c/...).
    "$ADB" install -r "$(cygpath -w "$GEN/app/build/outputs/apk/x86_64/debug/app-x86_64-debug.apk")"
    "$ADB" shell pm grant app.haze.player android.permission.READ_MEDIA_AUDIO
    "$ADB" shell pm grant app.haze.player android.permission.POST_NOTIFICATIONS
    "$ADB" shell monkey -p app.haze.player -c android.intent.category.LAUNCHER 1 >/dev/null
    ;;
  devtools)
    P=$("$ADB" shell pidof app.haze.player | tr -d '\r')
    "$ADB" forward --remove-all
    "$ADB" forward tcp:9222 "localabstract:webview_devtools_remote_$P" >/dev/null
    echo "DevTools de Haze (pid $P) en localhost:9222"
    ;;
  *)
    sed -n '2,15p' "$0"
    ;;
esac
