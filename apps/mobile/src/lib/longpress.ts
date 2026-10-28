// Mantener apretado: abre las opciones en vez del clic derecho de escritorio. Si se
// dispara, el clic de al soltar no cuenta en ningun lado (si no, caeria en el fondo de
// la hoja que se acaba de abrir debajo del dedo y la cerraria); si el dedo se mueve
// (scroll), no se dispara.
export function longpress(node: HTMLElement, handler: () => void) {
  let run = handler;
  let timer = 0;
  let x = 0;
  let y = 0;
  let fired = false;

  const clear = () => {
    clearTimeout(timer);
    timer = 0;
  };
  const down = (e: PointerEvent) => {
    fired = false;
    x = e.clientX;
    y = e.clientY;
    clear();
    timer = window.setTimeout(() => {
      timer = 0;
      fired = true;
      run();
    }, 450);
  };
  const move = (e: PointerEvent) => {
    if (timer && Math.hypot(e.clientX - x, e.clientY - y) > 10) clear();
  };
  const click = (e: MouseEvent) => {
    if (!fired) return;
    fired = false;
    e.preventDefault();
    e.stopImmediatePropagation();
  };
  // Un toque nuevo ya no es el del long press (por si ese clic nunca llego).
  const reset = () => (fired = false);

  node.addEventListener("pointerdown", down);
  node.addEventListener("pointermove", move);
  node.addEventListener("pointerup", clear);
  node.addEventListener("pointercancel", clear);
  window.addEventListener("click", click, true);
  window.addEventListener("pointerdown", reset, true);
  return {
    update(next: () => void) {
      run = next;
    },
    destroy() {
      clear();
      node.removeEventListener("pointerdown", down);
      node.removeEventListener("pointermove", move);
      node.removeEventListener("pointerup", clear);
      node.removeEventListener("pointercancel", clear);
      window.removeEventListener("click", click, true);
      window.removeEventListener("pointerdown", reset, true);
    },
  };
}
