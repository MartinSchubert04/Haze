# Haze para Samsung (Tizen)

La app de las teles Samsung (`apps/tizen`). A diferencia de Mythos, la app es entera local
(`index.html`, `app.js`, `app.css`, sin build): la tele no puede leer la música de la PC, así
que la biblioteca vive en el server y la sirve `apps/server` (el mismo contenedor que el bot de
Discord). El audio suena en la tele.

## Dónde va cada cosa

```
apps/tizen/
  config.xml     id HazeMusic1.Haze (el paquete tiene que tener 10 caracteres), privilegios
  index.html     las pantallas: elegir server, la app y "Sonando" (el tocadiscos)
  app.js         todo: control remoto, listas, cola, reproductor, letras
  app.css        tema Haze + el color de la tapa que suena (--tint)
  fonts/         Inter (la misma de escritorio)
  build.ps1      arma build\Haze.wgt firmado y, con -Tv <ip>, lo instala y lo abre
apps/server/src/web.rs   la biblioteca por HTTP (abajo)
```

## El server

`apps/server` sirve la música de `MUSIC_DIR` (en el contenedor, `/music`) en el puerto 7790:

| Ruta | Qué da |
|---|---|
| `GET /api/health` | `{ ok, app: "haze", songs }`: la tele lo usa para encontrarlo |
| `GET /api/library` | todas las canciones, con la carpeta relativa a la música |
| `GET /api/cover/{id}?s=600` | la tapa achicada a ese lado (cache en `DATA_DIR/covers`) |
| `GET /api/audio/{id}` | el archivo, con Range |
| `GET /api/lyrics/{id}` | el `.lrc` que está al lado del tema |
| `GET /api/yt/search?q=` | buscar en YouTube |
| `GET /api/yt/audio?url=` | escuchar de YouTube (el server pasa el audio, ver abajo) |
| `POST /api/yt/save` | bajar como MP3 a `MUSIC_DIR/YouTube` |
| `GET /api/yt/saves` | cómo van las descargas |
| `POST /api/rescan` | releer la carpeta (sola, cada 15 min) |

Sin login: es para la red de casa y el puerto no se abre en el router. El bot de Discord pasa
a ser opcional (sin `DISCORD_TOKEN` queda solo esto). El compose de ejemplo está en
`apps/server/docker-compose.example.yml`; el de verdad, en `docker-images/haze-bot` del server.

**La música** va en carpetas como las de la PC. Las de géneros pueden ser hardlinks de `All`:
la tele muestra una entrada por archivo en "Canciones" (como escritorio) y cada carpeta con lo
suyo. Para copiarla desde la PC conservando los hardlinks, un `.tar` (el `tarfile` de Python
los guarda) y `tar -x -C ~/music` en el server.

**YouTube.** `yt-dlp` da la URL directa del audio (m4a, solo formatos por HTTPS; los HLS no se
pueden pasar). Dos mañas de YouTube que resuelve el server:

- Rechaza el header `Range` (y los pedidos sin rango) con 403: hay que pedir de a pedazos con
  `&range=a-b` en la URL. El server traduce el `Range` de la tele a pedazos de 10 MB.
- Una URL recién pedida da 403 los primeros ~3 s y después anda: se reintenta la misma con una
  pausa (pedir otra no sirve, también es nueva).

## La app

- **Inicio**: mezclar todo, seguir escuchando, agregadas hace poco y las carpetas (mosaico de
  tapas). **Canciones**: todas, de la A a la Z. **Buscar**: en la biblioteca, mientras se
  escribe, y "Buscar en YouTube". **YouTube**: buscar, escuchar y **Guardar** en el server.
- **Listas largas** (`VList`): dibujan solo las filas que se ven; mantener apretado acelera
  (de a 1, 3 y 8) y CH+/CH- pasan de a una pantalla. A la derecha de cada fila, "A la cola" (o
  "Guardar" en YouTube).
- **La cola** como en escritorio: el contexto más lo encolado a mano, que suena antes.
  Aleatorio y repetir (todo / este tema). Se guarda en la tele y al volver aparece "Seguir
  escuchando".
- **Sonando**: se abre al poner un tema. El sobre con la tapa y el disco asomando, con la tapa
  como etiqueta. El disco gira a 33⅓ (Web Animations) y acelera y frena al reproducir y pausar;
  el brillo del vinilo queda quieto (como la luz de verdad). El brazo entra al disco y avanza
  con el tema. Al cambiar de tema el brazo se levanta, el disco vuelve al sobre, cambia la tapa
  y sale el nuevo. El fondo es la tapa desenfocada y el color de acento sale de la tapa.
  Con letra (`.lrc`), la línea que suena queda en el medio; sin tiempos, baja pareja con el tema.
  Sin tocar el control 9 s, se esconden los controles y queda el cuadro.
- **Teclas**: play/pausa, adelantar y retroceder (mantenidas, el salto crece), siguiente y
  anterior (y CH+/CH- en "Sonando"); se registran con `tizen.tvinputdevice`. Atrás: cierra
  "Sonando", sale de la carpeta, vuelve a la barra lateral, al inicio y recién ahí sale.
- **Protector de pantalla** apagado mientras suena (`webapis.appcommon`).
- **Elegir server**: se escribe la IP (sin `http://` ni puerto usa `:7790`) o **Buscar en esta
  red**, que prueba `/api/health` en las 254 direcciones de la red de la tele.

## Rendimiento

La UN50DU7000 rasteriza por CPU (ver el `docs/tizen.md` de Mythos): sin `backdrop-filter`, el
fondo desenfocado se dibuja una vez por tema, lo que se mueve siempre (disco, resplandor,
ecualizador) es `transform`/`opacity` en su propia capa, el foco no se anima y las listas y
filas se corren de un salto. Mientras "Sonando" está abierto, la app de abajo no se dibuja.

## Compilar, firmar e instalar

Usa las mismas herramientas y certificados que Mythos (Tizen Studio CLI y el perfil de firma
`Mythos`, ver el `docs/tizen.md` de Mythos): el certificado de distribuidor nombra a la tele, no
a la app.

```
cd apps/tizen
.\build.ps1                       # build\Haze.wgt firmado
.\build.ps1 -Tv 192.168.0.139     # además: sdb connect, instalar y abrir
```

Para probar en la PC: el server con `MUSIC_DIR`, `DATA_DIR` y `HAZE_WEB_BIND=127.0.0.1`
(`cargo run -p haze-bot`), y `apps/tizen/index.html?server=127.0.0.1` en Chrome (flechas,
Enter, Esc = atrás, espacio = play/pausa).
