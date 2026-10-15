import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";
import Mini from "./Mini.svelte";

// La misma pagina sirve para las dos ventanas: la mini se abre con #mini.
const target = document.getElementById("app")!;
const app = location.hash === "#mini" ? mount(Mini, { target }) : mount(App, { target });

// Sin menu contextual del navegador: la app tiene el suyo.
window.addEventListener("contextmenu", (e) => {
  const t = e.target as HTMLElement;
  if (!t.closest("input, textarea")) e.preventDefault();
});

export default app;
