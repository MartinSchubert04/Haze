// Haze para la tele (docs/tizen.md). Una sola pagina, sin build: la musica, las tapas y las
// letras las sirve el server de Haze (apps/server/src/web.rs) en la red de casa y suena
// en la tele con un <audio>.
//
// - Navegacion con el control: flechas por cercania entre los [data-f]; las listas largas
//   (.vlist) manejan arriba/abajo adentro y dibujan solo las filas que se ven.
// - La cola: el contexto (carpeta, todas, una busqueda) mas lo encolado a mano, que suena
//   antes y despues se retoma el contexto (como en escritorio).
// - "Sonando": el tocadiscos. El disco gira con la Web Animations API y acelera y frena
//   cambiando playbackRate; el brazo avanza con el tema; al cambiar de tema el disco
//   vuelve al sobre y sale el nuevo.
(function () {
  "use strict";

  const PORT = 7790;
  const KEY_SERVER = "haze.tv.server";
  const KEY_USER = "haze.tv.user";
  const KEY_SESSION = "haze.tv.session";
  const KEY_LYRICS = "haze.tv.lyrics";

  const K = {
    LEFT: 37, UP: 38, RIGHT: 39, DOWN: 40, ENTER: 13, BACK: 10009, ESC: 27,
    PLAYPAUSE: 10252, PLAY: 415, PAUSE: 19, STOP: 413, FF: 417, RW: 412,
    NEXT: 10233, PREV: 10232, CH_UP: 427, CH_DOWN: 428, IME_DONE: 65376, IME_CANCEL: 65385,
  };
  const MEDIA_KEYS = [
    "MediaPlayPause", "MediaPlay", "MediaPause", "MediaStop", "MediaFastForward", "MediaRewind",
    "MediaTrackNext", "MediaTrackPrevious", "ChannelUp", "ChannelDown",
  ];

  const $ = (s, r) => (r || document).querySelector(s);
  const $$ = (s, r) => Array.from((r || document).querySelectorAll(s));
  function h(tag, cls, text) {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text != null) e.textContent = text;
    return e;
  }
  const store = {
    get(k) { try { return localStorage.getItem(k); } catch (e) { return null; } },
    set(k, v) { try { localStorage.setItem(k, v); } catch (e) {} },
  };
  const hasTizen = typeof tizen !== "undefined";

  // ------------------------------------------------------------------ iconos

  const ICONS = {
    play: "M8 5.14v13.72a1 1 0 0 0 1.52.85l10.6-6.86a1 1 0 0 0 0-1.7L9.52 4.29A1 1 0 0 0 8 5.14z",
    pause: "M7 5h3.5v14H7zM13.5 5H17v14h-3.5z",
    next: "M5 6.5v11a1 1 0 0 0 1.55.83L14.5 13v4.5a.5.5 0 0 0 .5.5h2a.5.5 0 0 0 .5-.5v-11a.5.5 0 0 0-.5-.5h-2a.5.5 0 0 0-.5.5V11L6.55 5.67A1 1 0 0 0 5 6.5z",
    prev: "M19 6.5v11a1 1 0 0 1-1.55.83L9.5 13v4.5a.5.5 0 0 1-.5.5H7a.5.5 0 0 1-.5-.5v-11A.5.5 0 0 1 7 6h2a.5.5 0 0 1 .5.5V11l7.95-5.33A1 1 0 0 1 19 6.5z",
    shuffle: "M10.59 9.17 5.41 4 4 5.41l5.17 5.17 1.42-1.41zM14.5 4l2.04 2.04L4 18.59 5.41 20 17.96 7.46 20 9.5V4h-5.5zm.33 9.41-1.41 1.41 3.13 3.13L14.5 20H20v-5.5l-2.04 2.04-3.13-3.13z",
    repeat: "M7 7h10v3l4-4-4-4v3H5v6h2V7zm10 10H7v-3l-4 4 4 4v-3h12v-6h-2v4z",
    repeatOne: "M7 7h10v3l4-4-4-4v3H5v6h2V7zm10 10H7v-3l-4 4 4 4v-3h12v-6h-2v4zm-4-2V9h-1l-2 1v1h1.5v4H13z",
    home: "M10 20v-6h4v6h5v-8h3L12 3 2 12h3v8z",
    songs: "M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6z",
    search: "M15.5 14h-.79l-.28-.27A6.47 6.47 0 0 0 16 9.5 6.5 6.5 0 1 0 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z",
    youtube: "M21.6 7.2a2.5 2.5 0 0 0-1.76-1.77C18.27 5 12 5 12 5s-6.27 0-7.84.43A2.5 2.5 0 0 0 2.4 7.2 26 26 0 0 0 2 12a26 26 0 0 0 .4 4.8 2.5 2.5 0 0 0 1.76 1.77C5.73 19 12 19 12 19s6.27 0 7.84-.43a2.5 2.5 0 0 0 1.76-1.77A26 26 0 0 0 22 12a26 26 0 0 0-.4-4.8zM10 15V9l5.2 3z",
    disc: "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm0 14.5a4.5 4.5 0 1 1 0-9 4.5 4.5 0 0 1 0 9zm0-5.5a1 1 0 1 0 0 2 1 1 0 0 0 0-2z",
    lyrics: "M12 14a3 3 0 0 0 3-3V5a3 3 0 0 0-6 0v6a3 3 0 0 0 3 3zm5-3a5 5 0 0 1-10 0H5a7 7 0 0 0 6 6.92V21h2v-3.08A7 7 0 0 0 19 11h-2z",
    save: "M19 9h-4V3H9v6H5l7 7 7-7zM5 18v2h14v-2H5z",
    check: "M9 16.17 4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z",
    queue: "M14 10H3v2h11v-2zm0-4H3v2h11V6zm4 8v-4h-2v4h-4v2h4v4h2v-4h4v-2h-4zM3 16h7v-2H3v2z",
    server: "M4 4h16v6H4zm0 10h16v6H4zm3-7.5a1 1 0 1 0 0 1 1 1 0 0 0 0-1zm0 10a1 1 0 1 0 0 1 1 1 0 0 0 0-1z",
  };
  function icon(name) {
    return `<svg class="i" viewBox="0 0 24 24"><path d="${ICONS[name] || ""}"/></svg>`;
  }
  function paintIcons(root) {
    for (const el of $$("i[data-icon]", root)) el.innerHTML = icon(el.dataset.icon);
  }
  function setIcon(el, name) {
    const i = el.querySelector("i[data-icon]") || el;
    i.dataset.icon = name;
    i.innerHTML = icon(name);
  }

  // ------------------------------------------------------------------ formato

  const fmt = (s) => {
    s = Math.max(0, Math.floor(s || 0));
    const m = Math.floor(s / 60);
    const r = s % 60;
    return m >= 60 ? `${Math.floor(m / 60)}:${String(m % 60).padStart(2, "0")}:${String(r).padStart(2, "0")}` : `${m}:${String(r).padStart(2, "0")}`;
  };
  const fold = (s) => (s || "").normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  const plural = (n, one, many) => `${n.toLocaleString("es-AR")} ${n === 1 ? one : many}`;

  /** Saca lo que agregan las descargas ("(MP3_160K)", "(Official Video)"...). */
  function clean(s) {
    return (s || "")
      .replace(/\s*\((?:mp3|m4a|aac)_\d+k\)\s*$/i, "")
      .replace(/\s*[([][^)\]]*\b(?:official|oficial|video|audio|lyrics?|letra|visuali[sz]er|hd|hq|4k|remaster(?:ed)?)\b[^)\]]*[)\]]/gi, "")
      .replace(/\s{2,}/g, " ")
      .trim();
  }
  /** Titulo y artista para mostrar. Sin artista en los tags, "Artista - Tema" del titulo. */
  function prep(song) {
    let title = clean(song.title) || song.title || "";
    let artist = (song.artist || "").trim();
    // "Artista - Tema" en el titulo: se separa si no hay artista, si el artista es un canal
    // ("Calle13VEVO") o si es el mismo nombre.
    const m = /^(.+?)\s+[-–—]\s+(.+)$/.exec(title);
    if (m) {
      const squash = (s) => fold(s).replace(/vevo$|official|oficial|[^a-z0-9]/g, "");
      const a = squash(artist);
      const b = squash(m[1]);
      if (!artist || /vevo$/i.test(artist) || (a && b && (a.includes(b) || b.includes(a)))) {
        artist = m[1].trim();
        title = m[2].trim();
      }
    }
    let album = clean(song.album);
    if (album && (fold(album).includes(fold(title)) || fold(album) === fold(artist))) album = "";
    song.t = title;
    song.a = artist;
    song.al = album;
    song.key = fold(`${title} ${artist} ${song.album || ""} ${song.folder || ""}`);
    return song;
  }

  /** Un color de reemplazo para lo que no tiene tapa, siempre el mismo por titulo. */
  function hue(s) {
    let x = 0;
    for (let i = 0; i < s.length; i++) x = (x * 31 + s.charCodeAt(i)) | 0;
    return Math.abs(x) % 360;
  }

  // ------------------------------------------------------------------ server

  let server = "";
  let user = "default"; // de quien son las listas y bajas (como el usuario de escritorio)

  /** Como lo guarda el server (`haze_core::sync::clean_user`). */
  function cleanUser(s) {
    const u = String(s || "").trim().toLowerCase().replace(/[^a-z0-9_-]/g, "").slice(0, 32);
    return u || "default";
  }

  function api(path) {
    return server + path;
  }
  async function getJson(path, ms) {
    const c = new AbortController();
    const t = setTimeout(() => c.abort(), ms || 15000);
    try {
      const r = await fetch(api(path), { signal: c.signal });
      const j = await r.json().catch(() => null);
      if (!r.ok) throw new Error((j && j.error) || `HTTP ${r.status}`);
      return j;
    } finally {
      clearTimeout(t);
    }
  }
  function coverUrl(song, size) {
    if (song.kind === "stream") return song.thumb || "";
    return song.cover ? api(`/api/cover/${song.id}?s=${size || 300}`) : "";
  }
  function audioUrl(song) {
    if (song.kind === "stream") return api(`/api/yt/audio?url=${encodeURIComponent(song.path)}`);
    return api(`/api/audio/${song.id}`);
  }

  /** La tapa (o un reemplazo con color) dentro de `el`. */
  function art(el, song, size) {
    el.classList.add("art");
    el.innerHTML = "";
    const url = song && coverUrl(song, size);
    el.style.setProperty("--ph", `hsl(${hue((song && song.t) || "")} 22% 26%)`);
    if (url) {
      const img = new Image();
      img.decoding = "async";
      img.alt = "";
      img.src = url;
      img.onerror = () => {
        img.remove();
        el.appendChild(placeholder());
      };
      el.appendChild(img);
    } else {
      el.appendChild(placeholder());
    }
    return el;
  }
  function placeholder() {
    const p = h("span", "ph");
    p.innerHTML = icon("songs");
    return p;
  }

  // ------------------------------------------------------------------ biblioteca

  let raw = []; // lo que dio el server, con las dadas de baja
  let userData = null; // { lists, removed } de `user` (null: server viejo o no contesto)
  let all = []; // todas las entradas (el mismo tema puede estar en varias carpetas)
  let byId = new Map();
  let songs = []; // una por archivo, de la A a la Z
  let folders = []; // { name, songs }
  let lists = []; // las listas de Haze: { name, songs }

  async function loadLibrary() {
    const [j, u] = await Promise.all([getJson("/api/library", 30000), fetchUser()]);
    raw = j.songs.map((s) => prep(Object.assign(s, { kind: "local" })));
    userData = u;
    build();
  }

  /** Las listas y bajas del usuario. Si falla (server viejo), sin listas ni bajas. */
  async function fetchUser() {
    try {
      const u = await getJson(`/api/user/${encodeURIComponent(user)}`, 15000);
      return u && Array.isArray(u.lists) ? u : null;
    } catch (e) {
      return null;
    }
  }

  /** Canciones, carpetas y listas a partir de `raw` y `userData`. */
  function build() {
    // Las dadas de baja no se ven en ningun lado (el archivo sigue en el server).
    const removed = new Set(((userData && userData.removed) || []).filter((r) => r.removed).map((r) => r.file));
    all = removed.size ? raw.filter((s) => !removed.has(s.file)) : raw.slice();
    byId = new Map(all.map((s) => [s.id, s]));
    // Una por archivo: la de la carpeta mas especifica, como en escritorio.
    const depth = (s) => (s.folder.match(/\//g) || []).length;
    const pick = new Map();
    for (const s of all) {
      const cur = pick.get(s.file);
      if (!cur || depth(s) > depth(cur)) pick.set(s.file, s);
    }
    const coll = new Intl.Collator("es", { sensitivity: "base", numeric: true });
    songs = Array.from(pick.values()).sort((a, b) => coll.compare(a.t, b.t));
    const groups = new Map();
    for (const s of all) {
      const name = s.folder || "Sueltas";
      if (!groups.has(name)) groups.set(name, []);
      groups.get(name).push(s);
    }
    folders = Array.from(groups, ([name, list]) => ({
      name,
      label: name.split("/").pop(),
      songs: list.sort((a, b) => coll.compare(a.t, b.t)),
    })).sort((a, b) => coll.compare(a.label, b.label));
    // Listas: claves de archivo en su orden; las que no estan en el server no aparecen.
    const byFile = new Map(songs.map((s) => [s.file, s]));
    lists = ((userData && userData.lists) || [])
      .filter((l) => !l.deleted)
      .map((l) => ({
        name: l.name,
        songs: (l.songs || []).map((k) => byFile.get(k)).filter(Boolean),
      }))
      .sort((a, b) => coll.compare(a.name, b.name));
  }

  /** Volver a pedir las listas y bajas (otro aparato las pudo cambiar). */
  async function refreshUser() {
    if (!server || !raw.length) return;
    const u = await fetchUser();
    if (!u || JSON.stringify(u) === JSON.stringify(userData)) return;
    userData = u;
    build();
    redrawLibrary();
  }

  /** Hasta cuatro tapas distintas de una lista (para los mosaicos). */
  function covers(list, n) {
    const out = [];
    const seen = new Set();
    for (const s of list) {
      if (!s.cover || seen.has(s.file)) continue;
      seen.add(s.file);
      out.push(s);
      if (out.length >= n) break;
    }
    return out;
  }
  function mosaic(el, list) {
    el.innerHTML = "";
    const four = covers(list, 4);
    el.classList.toggle("one", four.length < 4);
    const use = four.length < 4 ? four.slice(0, 1) : four;
    if (!use.length) {
      art(el, { t: list[0] ? list[0].t : "" });
      return el;
    }
    for (const s of use) {
      const img = new Image();
      img.alt = "";
      img.decoding = "async";
      img.src = coverUrl(s, 300);
      el.appendChild(img);
    }
    return el;
  }

  // ------------------------------------------------------------------ foco

  let layer = "app"; // app | np | connect
  const lastFocus = {};

  function layerEl() {
    return layer === "np" ? $("#np") : layer === "connect" ? $("#connect") : $("#app");
  }
  function focusables() {
    return $$("[data-f]", layerEl()).filter(
      (el) => !el.disabled && el.offsetParent !== null && !(el._vl && !el._vl.items.length),
    );
  }
  function focus(el) {
    if (!el) return;
    if (el.tagName === "INPUT") quiet(el);
    el.focus({ preventScroll: true });
    reveal(el);
  }

  /** La distancia entre dos rangos (0 si se pisan). */
  function gap(a1, a2, b1, b2) {
    if (b2 < a1) return a1 - b2;
    if (b1 > a2) return b1 - a2;
    return 0;
  }
  /** Mueve el foco al [data-f] mas cercano en esa direccion. */
  function move(dir) {
    const cur = document.activeElement;
    const list = focusables();
    if (!list.includes(cur)) {
      focus(list[0]);
      return true;
    }
    // Volver de la barra lateral al contenido: a donde estaba.
    if (dir === "right" && cur.closest("#rail")) {
      const back = lastFocus[view];
      if (back && list.includes(back)) {
        focus(back);
        return true;
      }
    }
    const a = cur.getBoundingClientRect();
    let best = null;
    let bestScore = Infinity;
    for (const el of list) {
      if (el === cur) continue;
      const b = el.getBoundingClientRect();
      let main;
      let cross;
      if (dir === "left") {
        if (b.right > a.left + 4) continue;
        main = a.left - b.right;
        cross = gap(a.top, a.bottom, b.top, b.bottom);
      } else if (dir === "right") {
        if (b.left < a.right - 4) continue;
        main = b.left - a.right;
        cross = gap(a.top, a.bottom, b.top, b.bottom);
      } else if (dir === "up") {
        if (b.bottom > a.top + 4) continue;
        main = a.top - b.bottom;
        cross = gap(a.left, a.right, b.left, b.right);
      } else {
        if (b.top < a.bottom - 4) continue;
        main = b.top - a.bottom;
        cross = gap(a.left, a.right, b.left, b.right);
      }
      const score = Math.max(0, main) + cross * 3;
      if (score < bestScore) {
        bestScore = score;
        best = el;
      }
    }
    // Entrar a la barra lateral: a la seccion abierta, no a la que quede mas cerca.
    if (best && dir === "left" && best.closest("#rail") && !cur.closest("#rail")) {
      best = $("#rail .rail-item.active") || best;
    }
    if (best) focus(best);
    return !!best;
  }

  /** Que se vea lo enfocado: el inicio y las filas de tarjetas se corren de un salto. */
  function reveal(el) {
    const track = el.closest(".row-track");
    if (track) {
      const row = track.parentElement;
      const x = el.offsetLeft;
      let off = track._x || 0;
      if (x - off < 0) off = x;
      if (x + el.offsetWidth - off > row.clientWidth - 20) off = x + el.offsetWidth - row.clientWidth + 20;
      off = Math.max(0, off);
      track._x = off;
      track.style.transform = `translateX(${-off}px)`;
    }
    const home = el.closest(".home");
    if (home) {
      const viewEl = home.parentElement;
      const top = el.getBoundingClientRect().top - home.getBoundingClientRect().top;
      const max = Math.max(0, home.scrollHeight - viewEl.clientHeight + 120);
      const y = top < 320 ? 0 : Math.min(max, top - 300);
      home.style.transform = `translateY(${-y}px)`;
    }
  }

  // Campos de texto: el teclado de la tele solo se abre con el centro (como en Mythos).
  function quiet(input) {
    input.readOnly = true;
  }
  function type(input) {
    // Blur y focus de nuevo para que la tele abra el teclado; el blur lo vuelve a dejar de
    // solo lectura, por eso se habilita despues.
    input.blur();
    input.readOnly = false;
    input.focus();
  }

  // ------------------------------------------------------------------ listas largas

  const ROW_H = 104;

  class VList {
    constructor(el, opt) {
      this.el = el;
      this.opt = opt;
      this.items = [];
      this.sel = 0;
      this.sub = 0;
      this.top = 0;
      this.rows = new Map();
      this.track = h("div", "vlist-track");
      el.appendChild(this.track);
      el._vl = this;
      this.lastKey = 0;
      this.streak = 0;
    }
    setItems(items, keep) {
      this.items = items;
      this.sel = keep ? Math.min(this.sel, items.length - 1) : 0;
      if (this.sel < 0) this.sel = 0;
      if (!keep) this.top = 0;
      this.sub = 0;
      this.redraw();
    }
    redraw() {
      for (const r of this.rows.values()) r.remove();
      this.rows.clear();
      this.render();
    }
    render() {
      const vis = Math.max(1, Math.floor(this.el.clientHeight / ROW_H));
      const n = this.items.length;
      const m = Math.min(2, Math.floor((vis - 1) / 2));
      if (this.sel < this.top + m) this.top = this.sel - m;
      if (this.sel > this.top + vis - 1 - m) this.top = this.sel - (vis - 1 - m);
      this.top = Math.max(0, Math.min(this.top, n - vis));
      const from = Math.max(0, this.top - 1);
      const to = Math.min(n, this.top + vis + 1);
      for (const [i, r] of this.rows) {
        if (i < from || i >= to) {
          r.remove();
          this.rows.delete(i);
        }
      }
      for (let i = from; i < to; i++) {
        let r = this.rows.get(i);
        if (!r) {
          r = this.opt.row(this.items[i], i);
          r.classList.add("vrow");
          r.style.transform = `translateY(${i * ROW_H}px)`;
          this.track.appendChild(r);
          this.rows.set(i, r);
        }
        r.classList.toggle("sel", i === this.sel);
        r.classList.toggle("sub", i === this.sel && this.sub === 1);
      }
      this.track.style.transform = `translateY(${-this.top * ROW_H}px)`;
    }
    current() {
      return this.items[this.sel];
    }
    /** Una tecla con la lista enfocada. false: que la maneje la navegacion general. */
    key(k, e) {
      const n = this.items.length;
      if (k === K.UP || k === K.DOWN || k === K.CH_UP || k === K.CH_DOWN) {
        if (!n) return false;
        let step = 1;
        if (k === K.CH_UP || k === K.CH_DOWN) {
          step = Math.max(1, Math.floor(this.el.clientHeight / ROW_H) - 1);
        } else {
          // Mantener apretado acelera: de a 1, despues de a 3 y de a 8.
          const now = performance.now();
          this.streak = now - this.lastKey < 180 ? this.streak + 1 : 0;
          this.lastKey = now;
          step = this.streak > 30 ? 8 : this.streak > 10 ? 3 : 1;
        }
        const up = k === K.UP || k === K.CH_UP;
        if (up && this.sel === 0) return false;
        if (!up && this.sel === n - 1) return false;
        this.sel = Math.max(0, Math.min(n - 1, this.sel + (up ? -step : step)));
        this.sub = 0;
        this.render();
        return true;
      }
      if (k === K.RIGHT && this.opt.action && n && this.sub === 0) {
        this.sub = 1;
        this.render();
        return true;
      }
      if (k === K.LEFT && this.sub === 1) {
        this.sub = 0;
        this.render();
        return true;
      }
      if (k === K.ENTER && n) {
        const item = this.current();
        if (this.sub === 1) this.opt.action(item, this.sel);
        else this.opt.enter(item, this.sel);
        return true;
      }
      return false;
    }
  }

  // Filas
  function songRow() {
    return (s, i) => {
      const r = h("div");
      r.dataset.id = s.id;
      const num = h("span", "num");
      if (isCurrent(s)) {
        r.classList.add("now");
        num.innerHTML = '<span class="eq"><i></i><i></i><i></i></span>';
      } else {
        num.textContent = i + 1;
      }
      r.appendChild(num);
      r.appendChild(art(h("div"), s, 120));
      const txt = h("div", "txt");
      txt.appendChild(h("b", null, s.t));
      txt.appendChild(h("small", null, s.a || "—"));
      r.appendChild(txt);
      r.appendChild(h("span", "album", s.al || ""));
      if (s.lyrics) r.appendChild(h("span", "badge", "Letra"));
      r.appendChild(h("span", "dur", s.durationMs ? fmt(s.durationMs / 1000) : ""));
      const act = h("span", "act");
      act.innerHTML = `${icon("queue")}<span>A la cola</span>`;
      r.appendChild(act);
      return r;
    };
  }
  function ytRow(s) {
    const r = h("div");
    if (isCurrent(s)) r.classList.add("now");
    const a = art(h("div"), s);
    a.style.width = "121px";
    r.appendChild(a);
    const txt = h("div", "txt");
    txt.appendChild(h("b", null, s.t));
    txt.appendChild(h("small", null, s.a || "YouTube"));
    r.appendChild(txt);
    r.appendChild(h("span", "dur", s.durationMs ? fmt(s.durationMs / 1000) : ""));
    const act = h("span", "act");
    act.innerHTML = saveLabel(s.path);
    r.appendChild(act);
    return r;
  }

  // ------------------------------------------------------------------ vistas

  let view = "home";
  let listCtx = null; // { name, songs, kind, back }
  let listVL;
  let searchVL;
  let ytVL;

  function showView(name, opts) {
    view = name;
    const id = name === "songs" || name === "folder" ? "v-list" : `v-${name}`;
    for (const v of $$(".view")) v.hidden = v.id !== id;
    const railName = name === "folder" ? "home" : name;
    for (const b of $$(".rail-item[data-view]")) b.classList.toggle("active", b.dataset.view === railName);
    if (name === "songs") openList({ name: "Canciones", kind: "Biblioteca", songs }, opts);
    if (name === "search" && searchVL) searchVL.render();
    if (name === "youtube" && ytVL) ytVL.render();
  }

  function openList(ctx, opts) {
    // La misma (o la misma recargada): queda en la fila donde estaba.
    const same = listCtx && listCtx.name === ctx.name && listCtx.kind === ctx.kind && (listCtx.songs === ctx.songs || !!(opts && opts.keepFocus));
    listCtx = ctx;
    $("#list-kind").textContent = ctx.kind;
    $("#list-title").textContent = ctx.name;
    const total = ctx.songs.reduce((t, s) => t + (s.durationMs || 0), 0) / 1000;
    const hours = Math.floor(total / 3600);
    const mins = Math.round((total % 3600) / 60);
    $("#list-sub").textContent = `${plural(ctx.songs.length, "canción", "canciones")} · ${hours ? `${hours} h ` : ""}${mins} min`;
    mosaic($("#list-art"), ctx.songs);
    $("#list-art").classList.add("tile-art");
    listVL.setItems(ctx.songs, same);
    if (!(opts && opts.keepFocus)) focus(ctx.songs.length ? $("#list-play") : $("#rail .rail-item.active"));
  }

  function renderHome() {
    const v = $("#v-home");
    v.innerHTML = "";
    const home = h("div", "home");
    v.appendChild(home);

    const hour = new Date().getHours();
    const hello = h("header", "hello");
    hello.appendChild(h("h1", null, hour < 6 ? "Buenas noches" : hour < 13 ? "Buen día" : hour < 20 ? "Buenas tardes" : "Buenas noches"));
    const counts = [plural(songs.length, "canción", "canciones")];
    if (lists.length) counts.push(plural(lists.length, "lista", "listas"));
    counts.push(plural(folders.length, "carpeta", "carpetas"));
    hello.appendChild(h("p", null, counts.join(" · ")));
    home.appendChild(hello);

    const quick = h("div", "quick");
    const card = (iconName, title, sub, onEnter, img) => {
      const b = h("button", "quick-card");
      b.dataset.f = "";
      const qi = h("span", "qi");
      if (img) art(qi, img, 200);
      else qi.innerHTML = icon(iconName);
      b.appendChild(qi);
      const t = h("span");
      t.appendChild(h("b", null, title));
      t.appendChild(h("small", null, sub));
      b.appendChild(t);
      b.onclick = onEnter;
      quick.appendChild(b);
      return b;
    };
    card("shuffle", "Mezclar todo", plural(songs.length, "canción", "canciones"), () => {
      playList(songs, -1, "Todas", true);
      openNp();
    });
    if (P.cur) {
      card("play", P.cur.t, "Seguir escuchando", () => {
        resume();
        openNp();
      }, P.cur);
    }
    card("youtube", "YouTube", "Buscá y escuchá cualquier tema", () => {
      showView("youtube");
      focus($("#yt-input"));
    });
    home.appendChild(quick);

    const recent = songs.slice().sort((a, b) => b.mtime - a.mtime).slice(0, 24);
    if (recent.length) {
      home.appendChild(h("h2", "section-title", "Agregadas hace poco"));
      const row = h("div", "row");
      const track = h("div", "row-track");
      recent.forEach((s, i) => {
        const c = h("button", "card");
        c.dataset.f = "";
        const a = art(h("div"), s, 300);
        c.appendChild(a);
        c.appendChild(h("b", null, s.t));
        c.appendChild(h("small", null, s.a || "—"));
        c.onclick = () => {
          playList(recent, i, "Agregadas hace poco");
          openNp();
        };
        track.appendChild(c);
      });
      row.appendChild(track);
      home.appendChild(row);
    }

    // Listas (de Haze, por usuario) y carpetas: los mismos mosaicos.
    const tiles = (title, items, kind, cls) => {
      if (!items.length) return;
      home.appendChild(h("h2", "section-title", title));
      const grid = h("div", cls ? `grid ${cls}` : "grid");
      for (const f of items) {
        const name = f.label || f.name;
        const t = h("button", "tile");
        t.dataset.f = "";
        t.appendChild(mosaic(h("div", "tile-art"), f.songs));
        t.appendChild(h("b", null, name));
        t.appendChild(h("small", null, plural(f.songs.length, "canción", "canciones")));
        t.onclick = () => {
          lastFocus.home = t;
          showView("folder");
          openList({ name, kind, songs: f.songs });
        };
        grid.appendChild(t);
      }
      home.appendChild(grid);
    };
    tiles("Listas", lists, "Lista", "lists");
    tiles("Carpetas", folders, "Carpeta");

    const srv = h("button", "btn");
    srv.dataset.f = "";
    const who = user === "default" ? "" : ` · ${user}`;
    srv.innerHTML = `${icon("server")}<span>${server.replace(/^https?:\/\//, "")}${who} · Cambiar de server</span>`;
    srv.style.margin = "10px 0 90px";
    srv.onclick = () => showConnect("", server);
    home.appendChild(srv);
  }

  // ------------------------------------------------------------------ buscar

  let searchQuery = "";
  function runSearch() {
    const q = fold($("#search-input").value.trim());
    searchQuery = $("#search-input").value.trim();
    const yt = $("#search-yt");
    yt.hidden = !q;
    $("span", yt).textContent = `Buscar «${searchQuery}» en YouTube`;
    if (!q) {
      searchVL.setItems([]);
      $("#search-empty").textContent = "Escribí con el teclado de la tele: apretá el centro del control sobre el campo.";
      return;
    }
    const words = q.split(/\s+/);
    const res = songs.filter((s) => words.every((w) => s.key.includes(w))).slice(0, 300);
    searchVL.setItems(res);
    $("#search-empty").textContent = res.length ? "" : "No hay nada así en la biblioteca.";
  }

  // ------------------------------------------------------------------ YouTube

  const saves = new Map(); // url -> estado de la descarga en el server
  let savesTimer = 0;

  function saveLabel(url) {
    const s = saves.get(url);
    if (!s) return `${icon("save")}<span>Guardar</span>`;
    if (s.status === "done") return `${icon("check")}<span>Guardada</span>`;
    if (s.status === "error") return `${icon("save")}<span>Reintentar</span>`;
    if (s.status === "converting") return `<span>Convirtiendo…</span>`;
    if (s.status === "downloading") return `<span>Guardando ${Math.round(s.progress * 100)}%</span>`;
    return `<span>En espera…</span>`;
  }

  async function ytSearch(q) {
    q = (q || "").trim();
    if (!q) return;
    $("#yt-input").value = q;
    const empty = $("#yt-empty");
    empty.innerHTML = `<span class="spinner"></span>Buscando «${q.replace(/[<&]/g, "")}»…`;
    ytVL.setItems([]);
    try {
      const res = await getJson(`/api/yt/search?q=${encodeURIComponent(q)}`, 70000);
      const list = res.map((s) => prep(Object.assign(s, { kind: "stream" })));
      ytVL.setItems(list);
      empty.textContent = list.length ? "" : "YouTube no devolvió nada.";
      if (list.length && view === "youtube") focus($("#yt-rows"));
    } catch (e) {
      empty.textContent = `No se pudo buscar: ${e.message}`;
    }
  }

  async function save(song) {
    const cur = saves.get(song.path);
    if (cur && cur.status !== "error") return;
    try {
      const r = await fetch(api("/api/yt/save"), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ url: song.path, title: song.t }),
      });
      const s = await r.json();
      if (!r.ok) throw new Error(s.error || `HTTP ${r.status}`);
      saves.set(song.path, s);
      toast(`Guardando «${song.t}» en el server…`);
      refreshSaves();
      pollSaves();
    } catch (e) {
      toast(`No se pudo guardar: ${e.message}`);
    }
  }
  function pollSaves() {
    if (savesTimer) return;
    savesTimer = setInterval(async () => {
      try {
        const list = await getJson("/api/yt/saves", 8000);
        let pending = false;
        let finished = false;
        for (const s of list) {
          const before = saves.get(s.url);
          if (s.status === "done" && before && before.status !== "done") {
            finished = true;
            toast(`«${s.title || "El tema"}» quedó guardado en YouTube`);
          }
          if (s.status === "error" && before && before.status !== "error") toast(`No se pudo guardar: ${s.error || "error"}`);
          saves.set(s.url, s);
          if (s.status === "queued" || s.status === "downloading" || s.status === "converting") pending = true;
        }
        refreshSaves();
        if (finished) reloadLibrary();
        if (!pending) {
          clearInterval(savesTimer);
          savesTimer = 0;
        }
      } catch (e) {
        // Se reintenta en la proxima vuelta.
      }
    }, 1500);
  }
  function refreshSaves() {
    for (const [i, r] of ytVL.rows) {
      const s = ytVL.items[i];
      const act = $(".act", r);
      if (s && act) act.innerHTML = saveLabel(s.path);
    }
    updateSaveButton();
  }
  async function reloadLibrary() {
    try {
      await loadLibrary();
      redrawLibrary();
    } catch (e) {
      // Queda la de antes.
    }
  }
  /** Despues de `build()`: el inicio, la busqueda y lo abierto, con lo nuevo. */
  function redrawLibrary() {
    const inHome = document.activeElement && document.activeElement.closest("#v-home");
    renderHome();
    if (inHome) focus($("#v-home [data-f]"));
    if (searchVL) {
      const a = document.activeElement;
      runSearch();
      if (a && a.id === "search-rows" && !searchVL.items.length) focus($("#search-input"));
    }
    if (view === "songs") openList({ name: "Canciones", kind: "Biblioteca", songs }, { keepFocus: true });
    else if (view === "folder" && listCtx) {
      const now =
        listCtx.kind === "Lista" ? lists.find((l) => l.name === listCtx.name)
        : listCtx.kind === "Carpeta" ? folders.find((f) => f.label === listCtx.name)
        : null;
      if (now) openList({ name: listCtx.name, kind: listCtx.kind, songs: now.songs }, { keepFocus: true });
    }
  }

  // ------------------------------------------------------------------ reproductor

  const audio = $("#audio");
  const P = { ctx: [], name: "", order: [], pos: -1, up: [], cur: null, shuffle: false, repeat: "off" };
  let resumeAt = 0;

  function isCurrent(s) {
    return !!P.cur && (P.cur === s || (P.cur.kind === "local" && s.kind === "local" ? P.cur.file === s.file : P.cur.path === s.path && !!s.path));
  }

  function shuffled(n, first) {
    const a = [];
    for (let i = 0; i < n; i++) if (i !== first) a.push(i);
    for (let i = a.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      const t = a[i];
      a[i] = a[j];
      a[j] = t;
    }
    if (first >= 0) a.unshift(first);
    return a;
  }

  /** Suena `list` desde `start` (-1: cualquiera). */
  function playList(list, start, name, shuffle) {
    if (!list.length) return;
    P.ctx = list.slice();
    P.name = name;
    P.up = [];
    if (shuffle !== undefined) P.shuffle = shuffle;
    if (P.shuffle) {
      P.order = shuffled(P.ctx.length, start >= 0 ? start : Math.floor(Math.random() * P.ctx.length));
      P.pos = 0;
    } else {
      P.order = P.ctx.map((_, i) => i);
      P.pos = Math.max(0, start);
    }
    load(P.ctx[P.order[P.pos]], true);
  }

  function load(song, play) {
    const changed = P.cur !== song;
    P.cur = song;
    resumeAt = 0;
    audio.src = audioUrl(song);
    if (play) {
      const p = audio.play();
      if (p && p.catch) p.catch(() => {});
    }
    onSongChange(changed);
    saveSession();
  }

  function next(auto) {
    if (!P.cur) return;
    if (auto && P.repeat === "one") {
      audio.currentTime = 0;
      audio.play();
      return;
    }
    if (P.up.length) return load(P.up.shift(), true);
    let p = P.pos + 1;
    if (p >= P.order.length) {
      if (P.repeat !== "all" || !P.order.length) {
        if (auto) {
          audio.pause();
          audio.currentTime = 0;
        } else {
          toast("No hay más temas en la cola");
        }
        return;
      }
      if (P.shuffle) P.order = shuffled(P.ctx.length, -1);
      p = 0;
    }
    P.pos = p;
    load(P.ctx[P.order[p]], true);
  }

  function prev() {
    if (!P.cur) return;
    if (audio.currentTime > 4 || P.pos <= 0) {
      audio.currentTime = 0;
      return;
    }
    P.pos--;
    load(P.ctx[P.order[P.pos]], true);
  }

  function toggle() {
    if (!P.cur) return;
    if (audio.paused) resume();
    else audio.pause();
  }
  function resume() {
    if (!P.cur) return;
    if (!audio.src) audio.src = audioUrl(P.cur);
    const p = audio.play();
    if (p && p.catch) p.catch(() => {});
  }

  function setShuffle(on) {
    P.shuffle = on;
    if (!P.ctx.length) return updateModes();
    const cur = P.order[P.pos];
    if (on) {
      P.order = shuffled(P.ctx.length, cur);
      P.pos = 0;
    } else {
      P.order = P.ctx.map((_, i) => i);
      P.pos = cur;
    }
    updateModes();
    renderUpNext();
    saveSession();
  }
  function cycleRepeat() {
    P.repeat = P.repeat === "off" ? "all" : P.repeat === "all" ? "one" : "off";
    updateModes();
    saveSession();
    toast(P.repeat === "off" ? "Repetir apagado" : P.repeat === "all" ? "Repetir todo" : "Repetir este tema");
  }
  function enqueue(song) {
    if (!P.cur) {
      playList([song], 0, "Cola");
      return;
    }
    P.up.push(song);
    renderUpNext();
    saveSession();
    toast(`«${song.t}» va a la cola`);
  }

  /** Lo que viene: lo encolado y despues el contexto. */
  function upcoming(n) {
    const out = P.up.slice(0, n);
    for (let p = P.pos + 1; out.length < n && p < P.order.length; p++) out.push(P.ctx[P.order[p]]);
    return out;
  }

  // Teclas de adelantar/retroceder: la tele no las repite al mantenerlas, el salto crece
  // con el tiempo hasta soltar (como en Mythos).
  let holdTimer = 0;
  function seekBy(d) {
    if (!P.cur || !isFinite(audio.duration)) return;
    audio.currentTime = Math.max(0, Math.min(audio.duration - 1, audio.currentTime + d));
    updateTime(true);
  }
  function holdSeek(dir) {
    stopHold();
    let step = 10;
    seekBy(dir * step);
    holdTimer = setInterval(() => {
      step = Math.min(60, step + 5);
      seekBy(dir * step);
    }, 350);
  }
  function stopHold() {
    if (holdTimer) clearInterval(holdTimer);
    holdTimer = 0;
  }

  audio.addEventListener("ended", () => next(true));
  audio.addEventListener("play", onPlayState);
  audio.addEventListener("pause", onPlayState);
  audio.addEventListener("waiting", () => $("#np").classList.add("buffering"));
  audio.addEventListener("playing", () => $("#np").classList.remove("buffering"));
  audio.addEventListener("canplay", () => $("#np").classList.remove("buffering"));
  audio.addEventListener("timeupdate", () => updateTime(false));
  audio.addEventListener("loadedmetadata", () => {
    if (resumeAt > 0 && resumeAt < audio.duration - 2) audio.currentTime = resumeAt;
    resumeAt = 0;
    updateTime(true);
  });
  let errorSkips = 0;
  let retried = null;
  audio.addEventListener("error", () => {
    if (!P.cur || !audio.src) return;
    // Un corte o una URL de YouTube rechazada: una vez mas antes de saltarlo.
    if (retried !== P.cur) {
      retried = P.cur;
      const at = audio.currentTime || 0;
      setTimeout(() => {
        resumeAt = at;
        audio.src = audioUrl(P.cur);
        resume();
      }, 800);
      return;
    }
    $("#np").classList.remove("buffering");
    toast(`No se puede reproducir «${P.cur.t}»`);
    // Si fallan varios seguidos (server caido), no recorre toda la cola.
    if (++errorSkips < 4) setTimeout(() => next(false), 1500);
  });
  audio.addEventListener("playing", () => (errorSkips = 0));

  // ------------------------------------------------------------------ pantalla: sonando

  const np = $("#np");
  const tt = $("#tt");
  const arm = $("#arm");
  const disc = $("#disc");
  let npShown = null; // la cancion que muestra el tocadiscos
  let swapToken = 0;
  let armRest = true;
  let underTimer = 0;

  function openNp() {
    if (!P.cur) return;
    if (layer === "app") lastFocus.beforeNp = document.activeElement;
    layer = "np";
    np.classList.remove("closing");
    np.hidden = false;
    // La app queda tapada: se esconde cuando termina de entrar (asi no se dibuja debajo).
    clearTimeout(underTimer);
    underTimer = setTimeout(() => {
      if (layer === "np") $("#app").classList.add("under");
    }, 560);
    if (npShown !== P.cur) {
      showSong(P.cur);
      armRest = true;
      setArm();
      setTimeout(() => {
        armRest = false;
        setArm();
      }, 450);
    }
    setSpin(audio.paused ? 0 : 1);
    updateTime(true);
    focus($("#c-play"));
    wake();
  }
  function closeNp() {
    if (layer !== "np") return;
    layer = "app";
    clearTimeout(underTimer);
    $("#app").classList.remove("under");
    // Escondidas no se pueden medir: las listas se vuelven a armar.
    for (const vl of [listVL, searchVL, ytVL]) vl.redraw();
    np.classList.add("closing");
    clearTimeout(idleTimer);
    setTimeout(() => {
      if (layer !== "np") np.hidden = true;
    }, 300);
    const back = lastFocus.beforeNp;
    focus(back && back.offsetParent !== null ? back : $("#mini").hidden ? $("#rail .rail-item.active") : $("#mini"));
  }

  /** Pone la tapa, los textos y la letra de `song` en el tocadiscos, sin animar. */
  function showSong(song) {
    npShown = song;
    const big = coverUrl(song, 600);
    for (const img of [$(".sleeve img"), $(".disc-label img")]) {
      img.style.visibility = big ? "visible" : "hidden";
      if (big) img.src = big;
      else img.removeAttribute("src");
    }
    const fb = $(".sleeve-fallback");
    fb.hidden = !!big;
    if (!big) {
      fb.style.background = `linear-gradient(135deg, hsl(${hue(song.t)} 30% 32%), hsl(${(hue(song.t) + 40) % 360} 30% 16%))`;
      fb.innerHTML = `<svg class="i" viewBox="0 0 24 24" style="width:180px;height:180px;opacity:.35"><path d="${ICONS.songs}"/></svg>`;
    }
    $("#np-title").textContent = song.t;
    $("#np-artist").textContent = song.a || (song.kind === "stream" ? "YouTube" : "");
    $("#np-album").textContent = song.al || (song.folder && song.kind === "local" ? song.folder.split("/").pop() : "");
    $("#np-context").textContent = P.name ? `Sonando · ${P.name}` : "Sonando";
    loadLyrics(song);
    renderUpNext();
    updateSaveButton();
  }

  /** Cambio de tema con el tocadiscos a la vista: el brazo se levanta, el disco vuelve al
   *  sobre, cambia la tapa y sale el nuevo. */
  function swapTo(song) {
    const token = ++swapToken;
    armRest = true;
    setArm();
    $(".np-side").classList.add("changing");
    setTimeout(() => {
      if (token === swapToken) tt.classList.add("swap");
    }, 120);
    setTimeout(() => {
      if (token !== swapToken) return;
      showSong(song);
      $(".np-side").classList.remove("changing");
    }, 600);
    setTimeout(() => {
      if (token === swapToken) tt.classList.remove("swap");
    }, 680);
    setTimeout(() => {
      if (token !== swapToken) return;
      armRest = false;
      setArm();
    }, 1300);
  }

  /** El brazo: afuera en reposo; tocando, de 8° (borde del disco) a 27° (la etiqueta). */
  function setArm() {
    let deg = -6;
    if (!armRest && P.cur) {
      const d = audio.duration;
      const p = isFinite(d) && d > 0 ? audio.currentTime / d : 0;
      deg = 8 + 19 * Math.min(1, p);
    }
    arm.style.transform = `rotate(${deg.toFixed(2)}deg)`;
  }

  // El giro: 33⅓ rpm = una vuelta cada 1,8 s. Arranca en ~0,9 s y frena en ~1,6 s.
  let spin = null;
  let spinRate = 0;
  let spinTarget = 0;
  let spinRaf = 0;
  let spinLast = 0;
  function setSpin(target) {
    if (!spin && disc.animate) {
      spin = disc.animate([{ transform: "rotate(0deg)" }, { transform: "rotate(360deg)" }], {
        duration: 1800,
        iterations: Infinity,
      });
      spin.pause();
    }
    if (!spin) return;
    spinTarget = target;
    if (!spinRaf) {
      spinLast = 0;
      spinRaf = requestAnimationFrame(stepSpin);
    }
  }
  function stepSpin(ts) {
    const dt = spinLast ? Math.min(0.05, (ts - spinLast) / 1000) : 0.016;
    spinLast = ts;
    if (spinRate < spinTarget) spinRate = Math.min(spinTarget, spinRate + dt / 0.9);
    else spinRate = Math.max(spinTarget, spinRate - dt / 1.6);
    // Suave al llegar a la velocidad y al detenerse.
    const eased = spinRate * spinRate * (3 - 2 * spinRate);
    if (eased > 0.001) {
      spin.playbackRate = eased;
      if (spin.playState !== "running") spin.play();
    } else {
      spin.pause();
    }
    if (spinRate !== spinTarget) spinRaf = requestAnimationFrame(stepSpin);
    else spinRaf = 0;
  }

  function onPlayState() {
    const playing = !audio.paused;
    document.body.classList.toggle("paused", !playing);
    setIcon($("#c-play"), playing ? "pause" : "play");
    setIcon($(".mini-state"), playing ? "pause" : "play");
    $("#mini").classList.toggle("mini-on", playing);
    if (!np.hidden) setSpin(playing ? 1 : 0);
    else {
      // Escondido no hace falta acelerar de a poco.
      spinRate = spinTarget = playing ? 1 : 0;
      if (spin) {
        spin.playbackRate = 1;
        if (playing) spin.play();
        else spin.pause();
      }
    }
    screensaver(!playing);
    if (playing) wake();
    saveSession();
  }

  function onSongChange(changed) {
    const s = P.cur;
    // Mini reproductor
    const mini = $("#mini");
    mini.hidden = !s;
    $("#rail-now").hidden = !s;
    $("#app").classList.toggle("playing-bar", !!s);
    if (s) {
      $(".mini-title").textContent = s.t;
      $(".mini-artist").textContent = s.a || (s.kind === "stream" ? "YouTube" : "");
      const img = $(".mini-label img");
      const url = coverUrl(s, 200);
      img.style.visibility = url ? "visible" : "hidden";
      if (url) img.src = url;
    }
    // Marcas de "sonando" en las listas
    for (const vl of [listVL, searchVL, ytVL]) if (vl) vl.redraw();
    if (changed && s) {
      setBackground(s);
      if (!np.hidden && npShown && npShown !== s) swapTo(s);
      else if (!np.hidden) showSong(s);
    }
    updateTime(true);
    renderUpNext();
  }

  let lastSec = -1;
  function updateTime(force) {
    const d = audio.duration;
    const t = audio.currentTime || 0;
    const dur = isFinite(d) && d > 0 ? d : (P.cur && P.cur.durationMs ? P.cur.durationMs / 1000 : 0);
    const p = dur ? Math.min(1, t / dur) : 0;
    $(".mini-progress div").style.transform = `scaleX(${p})`;
    if (np.hidden) return;
    $("#seek-fill").style.transform = `scaleX(${p})`;
    $("#seek-knob").style.left = `${p * 100}%`;
    const sec = Math.floor(t);
    if (force || sec !== lastSec) {
      lastSec = sec;
      $("#t-pos").textContent = fmt(t);
      $("#t-dur").textContent = fmt(dur);
      if (!armRest) setArm();
    }
    syncLyrics(t, p);
  }

  function updateModes() {
    const sh = $("#c-shuffle");
    sh.classList.toggle("on", P.shuffle);
    const rp = $("#c-repeat");
    rp.classList.toggle("on", P.repeat !== "off");
    setIcon(rp, P.repeat === "one" ? "repeatOne" : "repeat");
  }

  function renderUpNext() {
    const box = $("#upnext");
    box.innerHTML = "";
    const list = upcoming(4);
    if (!list.length) return;
    box.appendChild(h("h3", null, "A continuación"));
    for (const s of list) {
      const it = h("div", "upnext-item");
      it.appendChild(art(h("div"), s, 120));
      const tx = h("div");
      tx.style.minWidth = "0";
      tx.appendChild(h("b", null, s.t));
      tx.appendChild(h("small", null, s.a || ""));
      it.appendChild(tx);
      box.appendChild(it);
    }
  }

  function updateSaveButton() {
    const b = $("#c-save");
    const s = P.cur;
    b.hidden = !s || s.kind !== "stream";
    if (!b.hidden) {
      const st = saves.get(s.path);
      b.innerHTML = saveLabel(s.path);
      b.classList.toggle("on", !!st && st.status === "done");
    }
  }

  // Fondo y color de la tapa

  let bgIdx = 0;
  let tintToken = 0;
  function setBackground(song) {
    const url = coverUrl(song, 300);
    const layers = $$(".bg-art");
    const nextLayer = layers[(bgIdx + 1) % 2];
    const curLayer = layers[bgIdx % 2];
    const token = ++tintToken;
    if (!url) {
      curLayer.classList.remove("on");
      setTint(null);
      return;
    }
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => {
      if (token !== tintToken) return;
      $("img", nextLayer).src = url;
      nextLayer.classList.add("on");
      curLayer.classList.remove("on");
      bgIdx++;
      setTint(tintOf(img));
    };
    img.onerror = () => {
      if (token === tintToken) setTint(null);
    };
    img.src = url;
  }

  /** El color mas vivo de la tapa, aclarado para que se lea sobre el fondo oscuro. */
  function tintOf(img) {
    try {
      const c = document.createElement("canvas");
      c.width = c.height = 24;
      const ctx = c.getContext("2d");
      ctx.drawImage(img, 0, 0, 24, 24);
      const d = ctx.getImageData(0, 0, 24, 24).data;
      let best = null;
      let bestW = -1;
      const buckets = new Map();
      for (let i = 0; i < d.length; i += 4) {
        const [hh, ss, ll] = rgbToHsl(d[i], d[i + 1], d[i + 2]);
        if (ll < 0.12 || ll > 0.92) continue;
        const key = Math.round(hh / 20);
        const b = buckets.get(key) || { w: 0, h: 0, s: 0, n: 0 };
        const w = ss * ss * (1 - Math.abs(ll - 0.5));
        b.w += w;
        b.h += hh * w;
        b.s += ss * w;
        b.n++;
        buckets.set(key, b);
      }
      for (const b of buckets.values()) {
        if (b.w > bestW) {
          bestW = b.w;
          best = b;
        }
      }
      if (!best || best.w < 0.4) return null;
      const H = best.h / best.w;
      const S = Math.min(0.8, Math.max(0.35, best.s / best.w));
      return hslToRgb(H, S, 0.74);
    } catch (e) {
      return null; // tapa de otro origen sin CORS
    }
  }
  function setTint(rgb) {
    const c = rgb || [249, 226, 175];
    document.documentElement.style.setProperty("--tint", `rgb(${c.join(",")})`);
    document.documentElement.style.setProperty("--tint-rgb", c.join(" "));
  }
  function rgbToHsl(r, g, b) {
    r /= 255;
    g /= 255;
    b /= 255;
    const max = Math.max(r, g, b);
    const min = Math.min(r, g, b);
    const l = (max + min) / 2;
    if (max === min) return [0, 0, l];
    const d = max - min;
    const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    let hh;
    if (max === r) hh = (g - b) / d + (g < b ? 6 : 0);
    else if (max === g) hh = (b - r) / d + 2;
    else hh = (r - g) / d + 4;
    return [hh * 60, s, l];
  }
  function hslToRgb(hh, s, l) {
    const k = (n) => (n + hh / 30) % 12;
    const a = s * Math.min(l, 1 - l);
    const f = (n) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
    return [Math.round(f(0) * 255), Math.round(f(8) * 255), Math.round(f(4) * 255)];
  }

  // Letra

  let lyr = null; // { synced, lines, els, idx }
  let lyricsOn = store.get(KEY_LYRICS) !== "0";
  let lyricsToken = 0;

  async function loadLyrics(song) {
    const token = ++lyricsToken;
    lyr = null;
    const box = $("#lyrics");
    const inner = $("#lyrics-inner");
    inner.innerHTML = "";
    inner.style.transform = "";
    $("#c-lyrics").hidden = !song.lyrics;
    applyLyricsMode();
    if (!song.lyrics) return;
    try {
      const r = await fetch(api(`/api/lyrics/${song.id}`));
      if (!r.ok || token !== lyricsToken) return;
      const parsed = parseLrc(await r.text());
      if (token !== lyricsToken || !parsed.lines.length) return;
      lyr = parsed;
      lyr.idx = -1;
      lyr.els = parsed.lines.map((l) => {
        const p = h("p", null, l.text || "♪");
        inner.appendChild(p);
        return p;
      });
      box.classList.toggle("plain", !parsed.synced);
      applyLyricsMode();
      syncLyrics(audio.currentTime || 0, 0, true);
    } catch (e) {
      // Sin letra.
    }
  }
  function parseLrc(text) {
    const lines = [];
    let synced = false;
    for (const raw of text.split(/\r?\n/)) {
      const tags = Array.from(raw.matchAll(/\[(\d+):(\d+(?:[.:]\d+)?)\]/g));
      const t = raw.replace(/\[[^\]]*\]/g, "").trim();
      if (tags.length) {
        synced = true;
        for (const m of tags) lines.push({ t: Number(m[1]) * 60 + parseFloat(m[2].replace(":", ".")), text: t });
      } else if (t && !/^\[/.test(raw.trim())) {
        lines.push({ t: -1, text: t });
      }
    }
    if (synced) {
      const out = lines.filter((l) => l.t >= 0).sort((a, b) => a.t - b.t);
      return { synced: true, lines: out };
    }
    return { synced: false, lines };
  }
  function applyLyricsMode() {
    const has = !!lyr && !!P.cur && P.cur.lyrics;
    const show = has && lyricsOn;
    $("#lyrics").hidden = !show;
    $("#upnext").hidden = show;
    $("#c-lyrics").classList.toggle("on", lyricsOn);
  }
  function syncLyrics(t, p, force) {
    if (!lyr || $("#lyrics").hidden) return;
    const box = $("#lyrics");
    const inner = $("#lyrics-inner");
    if (!lyr.synced) {
      // Sin tiempos: baja parejo con el tema.
      const max = Math.max(0, inner.scrollHeight - box.clientHeight * 0.55);
      inner.style.transform = `translateY(${-Math.round(max * p)}px)`;
      return;
    }
    let i = -1;
    const lines = lyr.lines;
    // La linea se marca un poco antes: el ojo llega tarde.
    while (i + 1 < lines.length && lines[i + 1].t <= t + 0.25) i++;
    if (i === lyr.idx && !force) return;
    if (lyr.idx >= 0 && lyr.els[lyr.idx]) lyr.els[lyr.idx].className = "past";
    for (let k = Math.max(0, i - 1); k < lyr.els.length && k <= i + 1; k++) if (k > i) lyr.els[k].className = "";
    if (force) lyr.els.forEach((el, k) => (el.className = k < i ? "past" : ""));
    lyr.idx = i;
    const el = lyr.els[Math.max(0, i)];
    if (i >= 0) el.className = "now";
    const y = el.offsetTop + el.offsetHeight / 2 - box.clientHeight * 0.4;
    inner.style.transform = `translateY(${-Math.max(0, Math.round(y))}px)`;
  }

  // Ocio: sin tocar el control, se esconden los controles y queda el cuadro.
  let idleTimer = 0;
  function wake() {
    np.classList.remove("idle");
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => {
      if (layer === "np" && !audio.paused) np.classList.add("idle");
    }, 9000);
  }

  function screensaver(allow) {
    try {
      const ac = webapis.appcommon;
      ac.setScreenSaver(allow ? ac.AppCommonScreenSaverState.SCREEN_SAVER_ON : ac.AppCommonScreenSaverState.SCREEN_SAVER_OFF);
    } catch (e) {
      // Fuera de la tele.
    }
  }

  // ------------------------------------------------------------------ sesion

  let sessionTimer = 0;
  function saveSession() {
    clearTimeout(sessionTimer);
    sessionTimer = setTimeout(() => {
      if (!P.cur) return;
      const ref = (s) => (s.kind === "local" ? s.id : { kind: "stream", path: s.path, title: s.title, artist: s.artist, durationMs: s.durationMs, thumb: s.thumb, id: s.id });
      const data = {
        ctx: P.ctx.map(ref),
        name: P.name,
        order: P.order,
        pos: P.pos,
        up: P.up.map(ref),
        cur: ref(P.cur),
        shuffle: P.shuffle,
        repeat: P.repeat,
        t: audio.currentTime || 0,
      };
      store.set(KEY_SESSION, JSON.stringify(data));
    }, 400);
  }
  setInterval(() => {
    if (P.cur && !audio.paused) saveSession();
  }, 10000);

  function restoreSession() {
    let d;
    try {
      d = JSON.parse(store.get(KEY_SESSION) || "null");
    } catch (e) {
      d = null;
    }
    if (!d || !d.cur) return;
    const back = (r) => (typeof r === "number" ? byId.get(r) : r && r.path ? prep(Object.assign({}, r)) : null);
    const cur = back(d.cur);
    if (!cur) return;
    P.ctx = (d.ctx || []).map(back).filter(Boolean);
    P.order = Array.isArray(d.order) && d.order.length === P.ctx.length ? d.order : P.ctx.map((_, i) => i);
    P.pos = Math.max(0, Math.min(d.pos || 0, P.order.length - 1));
    P.up = (d.up || []).map(back).filter(Boolean);
    P.name = d.name || "";
    P.shuffle = !!d.shuffle;
    P.repeat = d.repeat || "off";
    P.cur = cur;
    audio.preload = "metadata";
    audio.src = audioUrl(cur);
    resumeAt = d.t || 0;
    onSongChange(true);
    updateModes();
    onPlayState();
  }

  // ------------------------------------------------------------------ aviso

  let toastTimer = 0;
  function toast(msg) {
    const t = $("#toast");
    t.textContent = msg;
    t.classList.add("on");
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => t.classList.remove("on"), 3200);
  }

  // ------------------------------------------------------------------ conectar

  function normalize(s) {
    let t = String(s || "").trim().replace(/\/+$/, "");
    if (!t) return "";
    if (/^https?:\/\//i.test(t)) return t;
    t = `http://${t}`;
    if (!/:\d+$/.test(t.replace(/^https?:\/\//i, ""))) t += `:${PORT}`;
    return t;
  }

  async function probe(url, ms) {
    const c = new AbortController();
    const timer = setTimeout(() => c.abort(), ms);
    try {
      const r = await fetch(`${url}/api/health`, { signal: c.signal });
      const j = await r.json();
      return !!(j && j.ok && j.app === "haze");
    } catch (e) {
      return false;
    } finally {
      clearTimeout(timer);
    }
  }

  function showConnect(err, value) {
    layer = "connect";
    $("#app").hidden = true;
    np.hidden = true;
    $("#connect").hidden = false;
    $("#connect-error").textContent = err || "";
    if (value !== undefined) $("#addr").value = value.replace(/^http:\/\//, "").replace(/:7790$/, "");
    $("#user").value = user === "default" ? "" : user;
    focus(err ? $("#connect-go") : $("#addr"));
  }

  async function connect() {
    const url = normalize($("#addr").value);
    if (!/^https?:\/\/[^/\s]+$/i.test(url)) {
      $("#connect-error").textContent = "Eso no parece una dirección. Por ejemplo: 192.168.0.100";
      return;
    }
    const b = $("#connect-go");
    b.disabled = true;
    b.textContent = "Conectando…";
    const ok = await probe(url, 6000);
    b.disabled = false;
    b.textContent = "Conectar";
    if (!ok) {
      $("#connect-error").textContent = `No hay un server de Haze en ${url.replace(/^https?:\/\//, "")}.`;
      focus(b);
      return;
    }
    store.set(KEY_SERVER, url);
    user = cleanUser($("#user").value);
    store.set(KEY_USER, user);
    start(url);
  }

  function localIp() {
    return new Promise((done) => {
      if (!hasTizen || !tizen.systeminfo) return done("");
      const props = ["WIFI_NETWORK", "ETHERNET_NETWORK"];
      const nextProp = (i) => {
        if (i >= props.length) return done("");
        try {
          tizen.systeminfo.getPropertyValue(
            props[i],
            (p) => (p && p.ipAddress && p.ipAddress !== "0.0.0.0" ? done(p.ipAddress) : nextProp(i + 1)),
            () => nextProp(i + 1),
          );
        } catch (e) {
          nextProp(i + 1);
        }
      };
      nextProp(0);
    });
  }

  async function find() {
    const b = $("#connect-find");
    const list = $("#found");
    if (b.disabled) return;
    b.disabled = true;
    b.textContent = "Buscando…";
    list.innerHTML = "";
    $("#connect-error").textContent = "";
    const ip = await localIp();
    const m = /^(\d+\.\d+\.\d+)\.(\d+)$/.exec(ip);
    if (!m) {
      b.disabled = false;
      b.textContent = "Buscar en esta red";
      $("#connect-error").textContent = "No se puede saber la red de esta tele. Escribí la dirección a mano.";
      return;
    }
    const pending = [];
    for (let i = 1; i < 255; i++) if (String(i) !== m[2]) pending.push(`${m[1]}.${i}`);
    let found = 0;
    const worker = async () => {
      while (pending.length) {
        const url = `http://${pending.shift()}:${PORT}`;
        if (await probe(url, 2500)) {
          found++;
          const btn = h("button", "btn", url.replace("http://", ""));
          btn.dataset.f = "";
          btn.onclick = () => {
            $("#addr").value = url;
            connect();
          };
          list.appendChild(btn);
          if (found === 1) focus(btn);
        }
      }
    };
    await Promise.all(Array.from({ length: 40 }, worker));
    b.disabled = false;
    b.textContent = "Buscar en esta red";
    if (!found) $("#connect-error").textContent = `No se encontró Haze en ${m[1]}.x:${PORT}. Escribí la dirección a mano.`;
  }

  // ------------------------------------------------------------------ teclas

  function back() {
    const a = document.activeElement;
    if (layer === "connect") {
      if (server && byId.size) {
        $("#connect").hidden = true;
        $("#app").hidden = false;
        layer = "app";
        focus($("#rail .rail-item.active"));
        return;
      }
      return exitApp();
    }
    if (layer === "np") return closeNp();
    if (view === "folder") {
      showView("home");
      focus(lastFocus.home || $("#rail .rail-item.active"));
      return;
    }
    if (!a || !a.closest("#rail")) {
      focus($("#rail .rail-item.active") || $("#rail .rail-item"));
      return;
    }
    if (view !== "home") {
      showView("home");
      focus($('#rail [data-view="home"]'));
      return;
    }
    exitApp();
  }

  function exitApp() {
    try {
      tizen.application.getCurrentApplication().exit();
    } catch (e) {
      window.close();
    }
  }

  const DIRS = { [K.LEFT]: "left", [K.RIGHT]: "right", [K.UP]: "up", [K.DOWN]: "down" };

  document.addEventListener("keydown", (e) => {
    const k = e.keyCode;
    const a = document.activeElement;
    const typing = a && a.tagName === "INPUT" && !a.readOnly;

    // Teclas multimedia: andan en cualquier pantalla.
    if (k === K.PLAYPAUSE || (k === 32 && !typing)) {
      e.preventDefault();
      return toggle();
    }
    if (k === K.PLAY) return resume();
    if (k === K.PAUSE || k === K.STOP) return audio.pause();
    if (k === K.NEXT) return next(false);
    if (k === K.PREV) return prev();
    if (k === K.FF || k === K.RW) {
      e.preventDefault();
      if (!e.repeat) holdSeek(k === K.FF ? 1 : -1);
      return;
    }

    if (layer === "np") {
      const wasIdle = np.classList.contains("idle");
      wake();
      if (wasIdle && k !== K.BACK && k !== K.ESC) {
        e.preventDefault();
        return; // la primera tecla solo despierta
      }
      if (k === K.CH_UP) return next(false);
      if (k === K.CH_DOWN) return prev();
    }

    if (k === K.BACK || (k === K.ESC && !typing)) {
      e.preventDefault();
      return back();
    }

    if (typing) {
      if (k === K.IME_DONE || k === K.ENTER) {
        e.preventDefault();
        quiet(a);
        submitInput(a);
      } else if (k === K.IME_CANCEL || k === K.ESC) {
        quiet(a);
      }
      return;
    }

    if (a && a._vl && a._vl.key(k, e)) {
      e.preventDefault();
      return;
    }

    if (k === K.ENTER) {
      e.preventDefault();
      if (a && a.tagName === "INPUT") return type(a);
      if (a && a.click) a.click();
      return;
    }

    if (a && a.id === "seek" && (k === K.LEFT || k === K.RIGHT)) {
      e.preventDefault();
      return seekBy(k === K.RIGHT ? 10 : -10);
    }

    const dir = DIRS[k];
    if (dir) {
      e.preventDefault();
      move(dir);
    }
  });
  document.addEventListener("keyup", (e) => {
    if (e.keyCode === K.FF || e.keyCode === K.RW) stopHold();
  });

  // Recordar donde estaba el foco en cada vista; la barra lateral cambia de vista al pasar.
  document.addEventListener("focusin", (e) => {
    const el = e.target;
    if (el.closest && el.closest("#main")) lastFocus[view] = el;
    if (el.classList && el.classList.contains("rail-item") && el.dataset.view && el.dataset.view !== view) {
      if (!(view === "folder" && el.dataset.view === "home")) showView(el.dataset.view, { keepFocus: true });
    }
  });

  function submitInput(input) {
    if (input.id === "addr" || input.id === "user") connect();
    else if (input.id === "search-input") {
      runSearch();
      if (searchVL.items.length) focus($("#search-rows"));
    } else if (input.id === "yt-input") ytSearch(input.value);
  }

  // ------------------------------------------------------------------ arranque

  function wire() {
    paintIcons(document);

    listVL = new VList($("#list-rows"), {
      row: songRow(),
      enter: (s, i) => {
        playList(listCtx.songs, i, listCtx.name, false);
        openNp();
      },
      action: (s) => enqueue(s),
    });
    searchVL = new VList($("#search-rows"), {
      row: songRow(),
      enter: (s, i) => {
        playList(searchVL.items, i, `Búsqueda: ${searchQuery}`, false);
        openNp();
      },
      action: (s) => enqueue(s),
    });
    ytVL = new VList($("#yt-rows"), {
      row: ytRow,
      enter: (s, i) => {
        playList(ytVL.items, i, "YouTube", false);
        openNp();
      },
      action: (s) => save(s),
    });

    $("#list-play").onclick = () => {
      if (!listCtx) return;
      playList(listCtx.songs, 0, listCtx.name, false);
      openNp();
    };
    $("#list-shuffle").onclick = () => {
      if (!listCtx) return;
      playList(listCtx.songs, -1, listCtx.name, true);
      openNp();
    };
    for (const b of $$(".rail-item[data-view]")) {
      b.onclick = () => {
        showView(b.dataset.view);
        const target = b.dataset.view === "search" ? $("#search-input") : b.dataset.view === "youtube" ? $("#yt-input") : null;
        if (target) {
          focus(target);
          type(target);
        } else move("right");
      };
    }
    $("#rail-now").onclick = openNp;
    $("#mini").onclick = openNp;
    $("#search-input").addEventListener("input", runSearch);
    $("#search-yt").onclick = () => {
      showView("youtube");
      ytSearch(searchQuery);
    };
    $("#c-play").onclick = toggle;
    $("#c-next").onclick = () => next(false);
    $("#c-prev").onclick = prev;
    $("#c-shuffle").onclick = () => setShuffle(!P.shuffle);
    $("#c-repeat").onclick = cycleRepeat;
    $("#seek").onclick = toggle;
    $("#c-lyrics").onclick = () => {
      lyricsOn = !lyricsOn;
      store.set(KEY_LYRICS, lyricsOn ? "1" : "0");
      applyLyricsMode();
      syncLyrics(audio.currentTime || 0, 0, true);
    };
    $("#c-save").onclick = () => P.cur && save(P.cur);
    $("#connect-go").onclick = connect;
    $("#connect-find").onclick = find;
    for (const input of $$("input")) input.addEventListener("blur", () => quiet(input));

    if (hasTizen && tizen.tvinputdevice) {
      let supported = null;
      try {
        supported = new Set(tizen.tvinputdevice.getSupportedKeys().map((x) => x.name));
      } catch (e) {}
      for (const key of MEDIA_KEYS) {
        if (supported && !supported.has(key)) continue;
        try {
          tizen.tvinputdevice.registerKey(key);
        } catch (e) {}
      }
    }
    // La tele pasa a otra app: se pausa (no hay reproduccion de fondo).
    document.addEventListener("visibilitychange", () => {
      if (document.hidden && !audio.paused) audio.pause();
      // Al volver: las listas pudieron cambiar en otro aparato.
      if (!document.hidden && layer !== "connect") refreshUser();
    });
    updateModes();
    setTint(null);
    onPlayState();
  }

  async function start(url) {
    server = url;
    $("#connect").hidden = true;
    try {
      await loadLibrary();
    } catch (e) {
      showConnect(`No se puede leer la biblioteca de ${url.replace(/^https?:\/\//, "")}: ${e.message}`, url);
      return;
    }
    layer = "app";
    $("#app").hidden = false;
    if (!P.cur) restoreSession();
    renderHome();
    showView("home");
    runSearch();
    focus($("#v-home .quick-card") || $("#rail .rail-item"));
  }

  wire();
  const saved = store.get(KEY_SERVER);
  const qu = /[?&]user=([^&]+)/.exec(location.search);
  user = cleanUser(qu ? decodeURIComponent(qu[1]) : store.get(KEY_USER));
  const q = /[?&]server=([^&]+)/.exec(location.search);
  if (q) start(normalize(decodeURIComponent(q[1])));
  else if (saved) start(saved);
  else showConnect("", "");
})();
