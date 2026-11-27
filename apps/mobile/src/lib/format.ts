// Lo de @haze/ui/format, pero en ingles: el telefono va en ingles y escritorio en castellano.
export * from "@haze/ui/format";

export function plural(n: number, one: string, many: string): string {
  return `${n.toLocaleString("en-US")} ${n === 1 ? one : many}`;
}

/** "1 song", "3 songs". */
export const songCount = (n: number) => plural(n, "song", "songs");
