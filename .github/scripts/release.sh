#!/usr/bin/env bash
# Sube archivos al Release del tag actual (vX.Y.Z), creandolo si todavia no existe. Lo
# usan los dos workflows: el que termina primero crea el Release y el otro suma lo suyo,
# asi cada version trae siempre el instalador de Windows y el APK.
#
#   bash .github/scripts/release.sh 0.2.0 Haze-0.2.0-setup.exe latest.json
set -euo pipefail

V="$1"
shift
TAG="$GITHUB_REF_NAME"

cat > notas.md <<NOTAS
**Windows:** bajá \`Haze-$V-setup.exe\` y abrilo. Se instala solo para tu usuario, sin pedir permisos de administrador.

**Android:** bajá \`Haze-$V-android.apk\` en el celular y abrilo. Android va a pedir permiso para instalar apps de esta fuente.

Si ya tenés Haze, no hace falta: se actualiza sola.
NOTAS

# --latest: las apps instaladas buscan latest.json y latest-android.json en el ultimo
# Release. Si el otro workflow ya lo creo, solo se suben los archivos.
if ! gh release view "$TAG" >/dev/null 2>&1; then
  gh release create "$TAG" --title "Haze $V" --notes-file notas.md --latest || gh release view "$TAG" >/dev/null
fi
# Primero los instaladores y despues latest*.json: si suben juntos, el JSON (chico)
# termina antes y las apps encuentran la version nueva con un APK que todavia no existe
# ("GitHub respondio 404" al actualizar).
BINARIOS=()
MANIFIESTOS=()
for f in "$@"; do
  case "$(basename "$f")" in
    latest*.json) MANIFIESTOS+=("$f") ;;
    *) BINARIOS+=("$f") ;;
  esac
done
if [ ${#BINARIOS[@]} -gt 0 ]; then gh release upload "$TAG" "${BINARIOS[@]}" --clobber; fi
if [ ${#MANIFIESTOS[@]} -gt 0 ]; then gh release upload "$TAG" "${MANIFIESTOS[@]}" --clobber; fi
