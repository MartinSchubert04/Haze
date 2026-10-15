// Color dominante de una tapa, para teñir la pantalla de reproduccion.
const cache = new Map<string, string | null>();

export async function dominantColor(url: string | null): Promise<string | null> {
  if (!url) return null;
  if (cache.has(url)) return cache.get(url)!;
  const color = await new Promise<string | null>((resolve) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => {
      try {
        const c = document.createElement("canvas");
        c.width = c.height = 24;
        const ctx = c.getContext("2d", { willReadFrequently: true })!;
        ctx.drawImage(img, 0, 0, 24, 24);
        const d = ctx.getImageData(0, 0, 24, 24).data;
        // Promedio pesado por saturacion: un fondo gris no le gana a un color vivo.
        let r = 0, g = 0, b = 0, w = 0;
        for (let i = 0; i < d.length; i += 4) {
          const max = Math.max(d[i], d[i + 1], d[i + 2]);
          const min = Math.min(d[i], d[i + 1], d[i + 2]);
          const sat = max === 0 ? 0 : (max - min) / max;
          const light = max / 255;
          const weight = 0.08 + sat * sat * (light > 0.15 ? 1 : 0.2);
          r += d[i] * weight;
          g += d[i + 1] * weight;
          b += d[i + 2] * weight;
          w += weight;
        }
        resolve(`rgb(${Math.round(r / w)}, ${Math.round(g / w)}, ${Math.round(b / w)})`);
      } catch {
        resolve(null); // imagen sin CORS: sin tinte
      }
    };
    img.onerror = () => resolve(null);
    img.src = url;
  });
  cache.set(url, color);
  if (cache.size > 200) cache.delete(cache.keys().next().value!);
  return color;
}
