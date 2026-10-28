// Marca de "tocado" de los botones. No se usa :active porque en el WebView de Android a
// veces no aparece en un toque rapido, y :hover queda pegado despues del toque. Aca la
// clase `.pressed` se pone mientras el dedo esta apoyado, dura un minimo para que un toque
// rapido se vea, y al sacarla el CSS la apaga con un fade (ver app.css).

/** Lo que reacciona al tocar. El mas cercano al dedo gana: un boton dentro de otro marca solo el de adentro. */
const PRESSABLE = ".press, .icon-btn, .tap, .pill, .play, .swatch, .fold, .tab";

/** Espera antes de marcar, para no encender una fila cuando el dedo en realidad va a scrollear. */
const SHOW_DELAY = 70;
/** Lo minimo que queda marcado un toque rapido. */
const MIN_SHOWN = 140;
/** Si el dedo se mueve mas que esto, era un scroll y no un toque. */
const SLOP = 10;

export function installPress() {
  let el: HTMLElement | null = null;
  let shownAt = 0;
  let showTimer = 0;
  let x = 0;
  let y = 0;

  function show() {
    clearTimeout(showTimer);
    if (!el || el.classList.contains("pressed")) return;
    el.classList.add("pressed");
    shownAt = performance.now();
  }

  function release(cancel: boolean) {
    clearTimeout(showTimer);
    const target = el;
    el = null;
    if (!target) return;
    if (cancel) {
      target.classList.remove("pressed");
      return;
    }
    // Un toque mas rapido que la espera igual se marca, y se deja un momento.
    if (!target.classList.contains("pressed")) {
      target.classList.add("pressed");
      shownAt = performance.now();
    }
    const left = MIN_SHOWN - (performance.now() - shownAt);
    setTimeout(() => target.classList.remove("pressed"), Math.max(0, left));
  }

  window.addEventListener(
    "pointerdown",
    (e) => {
      if (e.button !== 0) return;
      release(true);
      const t = (e.target as Element | null)?.closest<HTMLElement>(PRESSABLE);
      if (!t || (t as HTMLButtonElement).disabled) return;
      el = t;
      x = e.clientX;
      y = e.clientY;
      if (e.pointerType === "mouse") show();
      else showTimer = window.setTimeout(show, SHOW_DELAY);
    },
    true,
  );
  window.addEventListener(
    "pointermove",
    (e) => {
      if (el && Math.hypot(e.clientX - x, e.clientY - y) > SLOP) release(true);
    },
    true,
  );
  window.addEventListener("pointerup", () => release(false), true);
  window.addEventListener("pointercancel", () => release(true), true);
  // El scroll del navegador no siempre manda pointercancel.
  window.addEventListener("scroll", () => release(true), true);
}
