// Cliente minimo de DevTools: node cdp.mjs eval "<js>" | shot <png> | logs <segundos>
import { writeFileSync } from "node:fs";
const [mode, arg] = process.argv.slice(2);
const pages = await (await fetch("http://localhost:9222/json")).json();
const ws = new WebSocket(pages.find((p) => p.type === "page").webSocketDebuggerUrl);
let id = 0;
const pending = new Map();
const send = (method, params = {}) =>
  new Promise((res) => {
    const i = ++id;
    pending.set(i, res);
    ws.send(JSON.stringify({ id: i, method, params }));
  });
ws.onmessage = (m) => {
  const d = JSON.parse(m.data);
  if (d.id && pending.has(d.id)) pending.get(d.id)(d.result ?? d.error);
  else if (d.method === "Runtime.consoleAPICalled")
    console.log("console." + d.params.type + ":", d.params.args.map((a) => a.value ?? a.description).join(" "));
  else if (d.method === "Runtime.exceptionThrown") {
    const x = d.params.exceptionDetails;
    console.log("EXCEPTION:", x.exception?.description ?? x.text);
  }
  else if (d.method === "Log.entryAdded") console.log("log:", d.params.entry.level, d.params.entry.text, d.params.entry.url ?? "");
};
await new Promise((r) => (ws.onopen = r));
if (mode === "eval") {
  const r = await send("Runtime.evaluate", { expression: arg, awaitPromise: true, returnByValue: true, timeout: 60000 });
  console.log(JSON.stringify(r.result?.value ?? r.exceptionDetails ?? r, null, 1));
} else if (mode === "shot") {
  const r = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(arg, Buffer.from(r.data, "base64"));
  console.log("ok", arg);
} else if (mode === "logs") {
  await send("Runtime.enable");
  await send("Log.enable");
  await new Promise((r) => setTimeout(r, Number(arg || 5) * 1000));
}
ws.close();
process.exit(0);
