// Mantener apretado: abre las opciones en vez del clic derecho de escritorio. Si se
// dispara, el toque de despues no cuenta como clic; si el dedo se mueve (scroll), no
// se dispara.
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

  node.addEventListener("pointerdown", down);
  node.addEventListener("pointermove", move);
  node.addEventListener("pointerup", clear);
  node.addEventListener("pointercancel", clear);
  node.addEventListener("click", click, true);
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
      node.removeEventListener("click", click, true);
    },
  };
}
