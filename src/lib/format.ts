export function time(seconds: number): string {
  if (!isFinite(seconds) || seconds < 0) seconds = 0;
  const s = Math.floor(seconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

export function folderName(path: string): string {
  return path.split("/").filter(Boolean).pop() ?? path;
}

export function plural(n: number, one: string, many: string): string {
  return `${n.toLocaleString("es-AR")} ${n === 1 ? one : many}`;
}

/** Igual que el celu: lo que empieza con letra va primero, lo raro al final. */
export function sortKey(text: string): string {
  const t = text
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .trim();
  return /^[a-z0-9]/.test(t) ? t : "￿" + t;
}

export function indexLetter(text: string): string {
  const c = sortKey(text)[0];
  if (!c || c === "￿") return "#";
  return /[a-z]/.test(c) ? c.toUpperCase() : "#";
}

export function normalizeSearch(text: string): string {
  return text
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase();
}
