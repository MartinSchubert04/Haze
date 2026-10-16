// Tirar hacia abajo para actualizar, como en las apps de Android: con la lista arriba de
// todo, arrastrar baja un circulo que gira con el dedo; si pasa el umbral al soltar,
// corre `refresh` y el circulo queda girando hasta que termina.

const THRESHOLD = 70;
const MAX = 110;

export function pullToRefresh(node: HTMLElement, refresh?: () => Promise<unknown>) {
  let run = refresh;
  let startY = 0;
  let pull = 0;
  let tracking = false;
  let busy = false;

  const ind = document.createElement("div");
  ind.className = "ptr";
  ind.innerHTML =
    '<svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M21 12a9 9 0 1 1-2.64-6.36"/><path d="M21 3v6h-6"/></svg>';
  document.body.appendChild(ind);

  function place(y: number, progress: number) {
    const top = node.getBoundingClientRect().top;
    // Debajo de la barra de estado, que tapa lo que pasa por arriba.
    ind.style.top = `calc(${top}px + var(--safe-top))`;
    ind.style.transform = `translate(-50%, ${y - 48}px) rotate(${progress * 270}deg)`;
    ind.style.opacity = String(Math.min(1, progress * 1.4));
    ind.classList.toggle("armed", progress >= 1);
  }

  function reset() {
    ind.classList.add("settle");
    place(0, 0);
    setTimeout(() => ind.classList.remove("settle", "spin"), 250);
  }

  function start(e: TouchEvent) {
    if (!run || busy || node.scrollTop > 0 || e.touches.length !== 1) return;
    startY = e.touches[0].clientY;
    pull = 0;
    tracking = true;
  }

  function move(e: TouchEvent) {
    if (!tracking) return;
    const dy = e.touches[0].clientY - startY;
    if (dy <= 0 || node.scrollTop > 0) {
      if (pull > 0) place(0, 0);
      pull = 0;
      return;
    }
    // Con resistencia: cuanto mas se tira, menos baja.
    pull = Math.min(MAX, dy * 0.5);
    e.preventDefault();
    place(pull, pull / THRESHOLD);
  }

  async function end() {
    if (!tracking) return;
    tracking = false;
    if (pull < THRESHOLD) {
      reset();
      return;
    }
    busy = true;
    ind.classList.add("settle", "spin");
    place(THRESHOLD, 1);
    try {
      await run?.();
    } finally {
      busy = false;
      reset();
    }
  }

  node.addEventListener("touchstart", start, { passive: true });
  node.addEventListener("touchmove", move, { passive: false });
  node.addEventListener("touchend", end);
  node.addEventListener("touchcancel", end);

  return {
    update(next?: () => Promise<unknown>) {
      run = next;
    },
    destroy() {
      node.removeEventListener("touchstart", start);
      node.removeEventListener("touchmove", move);
      node.removeEventListener("touchend", end);
      node.removeEventListener("touchcancel", end);
      ind.remove();
    },
  };
}
