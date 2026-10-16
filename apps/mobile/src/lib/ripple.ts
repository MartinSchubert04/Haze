// Onda al tocar, como la de Material: sale desde el dedo y se desvanece. Se dibuja en el
// hijo `.ripple-host` si hay (para no recortar lo que sobresale, como un label flotante);
// si no, en el elemento, que necesita `position: relative` y `overflow: hidden`.
export function ripple(node: HTMLElement) {
  function down(e: PointerEvent) {
    const host = node.querySelector<HTMLElement>(":scope > .ripple-host") ?? node;
    const r = host.getBoundingClientRect();
    const size = Math.hypot(r.width, r.height) * 2;
    const wave = document.createElement("span");
    wave.className = "ripple-wave";
    wave.style.width = wave.style.height = `${size}px`;
    wave.style.left = `${e.clientX - r.left - size / 2}px`;
    wave.style.top = `${e.clientY - r.top - size / 2}px`;
    host.appendChild(wave);
    const done = () => {
      wave.classList.add("out");
      setTimeout(() => wave.remove(), 400);
    };
    window.addEventListener("pointerup", done, { once: true });
    window.addEventListener("pointercancel", done, { once: true });
  }
  node.addEventListener("pointerdown", down);
  return { destroy: () => node.removeEventListener("pointerdown", down) };
}
