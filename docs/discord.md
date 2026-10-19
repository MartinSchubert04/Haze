# Bot de Discord

Haze de escritorio puede llevar un bot que entra al canal de voz donde estés y pone lo
mismo que suena en Haze. Los demás del canal suman temas a la misma cola con `/play`.

Apagado no usa nada: no hay conexión ni tareas. Prendido y sin canal, es una sola
conexión a Discord. El audio solo se arma con el bot dentro de un canal.

## Crear el bot (una vez)

1. Entrá a <https://discord.com/developers/applications> → **New Application** → ponele
   un nombre (por ejemplo "Haze").
2. **Bot** → **Reset Token** → copiá el token. No hace falta prender ningún "Privileged
   Gateway Intent".
3. En Haze: **Ajustes → Discord** → pegá el token → **Guardar** → prendé **Prender el bot**.
4. Cuando diga "Conectado", tocá **Sumar a un servidor** y elegí el servidor.

El bot sigue al dueño de la aplicación (la cuenta que la creó): cuando entrás a un
canal de voz de ese servidor, entra; cuando te vas, se va.

## Comandos

Solo funcionan para quien está en el mismo canal de voz que el bot.

| Comando | Qué hace |
| --- | --- |
| `/play <búsqueda o link>` | Busca en YouTube (o abre el link, también playlists) y lo suma a la cola |
| `/subir <archivo>` | Suma un archivo de audio propio |
| `/saltar` | Pasa al tema siguiente |
| `/pausa` | Pausa o sigue |
| `/cola` | Lo que viene |
| `/sonando` | Qué está sonando |

## Cómo funciona

- El motor de la PC (mpv) sigue mandando: el tiempo, el fin de cada tema y la cola salen
  de ahí. El bot repite cada orden (cargar, pausa, adelantar, volumen) en el canal.
- Con **Silenciar la PC** prendido, mientras el bot está en un canal los parlantes de la
  PC quedan en 0 y se escucha por Discord, sin eco.
- Archivos locales: los lee directo. YouTube y otros sitios: yt-dlp resuelve el audio
  (el mismo yt-dlp y deno de la app). Adjuntos de Discord: por HTTP.
- Los bots no pueden entrar a llamadas por mensaje directo ni a grupos: solo servidores.
- Un bot está en un solo canal por servidor; como sigue a una sola persona, no hay choques.

## Compilar

Opus (el códec de voz) se compila desde la fuente que trae `libopus_sys` y necesita
CMake. `.cargo/config.toml` corrige dónde lo instala en Windows (ver
`.cargo/opus-install.cmake`).
