# CLAUDE.md

## Versiones: commit, tag y push

Cada fix o feature terminado (probado, y con `cargo clean` hecho) se publica sin esperar a que lo
pidan: commitear, tagear y pushear `main` y el tag. El tag `v*` dispara `desktop.yml` y
`android.yml`, que arman el Release con el instalador de Windows y el APK.

- **Qué número sube** (semver, `MAYOR.MENOR.PARCHE`, sobre el último tag: `git tag --sort=-v:refname`):
  - `x.y.Z` (parche): arreglos y retoques chicos de algo que ya existía (un texto, un ícono, un bug).
  - `x.Y.0` (menor): algo nuevo que se puede usar (una pestaña, una opción, sincronizar), sin
    romper nada.
  - `X.0.0` (mayor): rompe algo de lo que ya existe (el formato de `settings.json`/`userdata.json`
    sin migración, la API del server que usan la tele o la sincronización). Antes de subirla, preguntar.
- **La versión sale del tag**: el CI la escribe en `Cargo.toml`/`tauri.conf.json` al compilar. Los
  números de esos archivos en el repo no se tocan.
- **Commit**: `feat: ...` o `fix: ...` (con `(desktop)`/`(mobile)` si es de una sola app), en
  español, con el cuerpo explicando el porqué (ver `git log`). Tag `vX.Y.Z` sobre el último commit
  de lo que se publica: `git tag vX.Y.Z && git push origin main vX.Y.Z`.
- Cambios que no son ni fix ni feature (docs, este archivo, CI) se commitean y pushean sin tag.

## Reglas

- **Espacio en disco de Rust.** En noviembre de 2026 `target/` llegó a ~30 GB y, junto con otros
  proyectos, llenó el C:. Para que no vuelva a pasar:
  - `--release` solo para publicar o probar el instalador.
  - En un worktree (`.claude/worktrees/`) compilar con `CARGO_TARGET_DIR` apuntando al `target/`
    del repo principal (`$env:CARGO_TARGET_DIR="<repo>\target"`), no crear uno nuevo. Al terminar
    la tarea, `git worktree remove` del worktree.
  - Al terminar las pruebas de una tarea que compiló (antes del push que la publica), `cargo clean`
    en `target/`: queda en 0. Volver a compilar tarda unos minutos, pero el disco no se llena.
    Antes, cerrar la instancia de prueba que corre desde ahí (Windows no deja borrar su .exe).
