import { mount } from "svelte";
import "@haze/ui/theme.css";
import "./app.css";
import App from "./App.svelte";

const app = mount(App, { target: document.getElementById("app")! });

// Mantener apretado abre las opciones de la app, no el menu del navegador.
window.addEventListener("contextmenu", (e) => {
  const t = e.target as HTMLElement;
  if (!t.closest("input, textarea")) e.preventDefault();
});

export default app;
