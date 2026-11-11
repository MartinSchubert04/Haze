# CLAUDE.md

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
