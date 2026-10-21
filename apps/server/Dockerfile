# Haze para Discord. Se arma desde la raiz del repo (necesita haze-core y el workspace):
#   docker build -f apps/server/Dockerfile .
# El compose esta en docker-images/haze-bot.

FROM rust:1-trixie AS build
RUN apt-get update && apt-get install -y --no-install-recommends cmake pkg-config libopus-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release -p haze-bot && cp target/release/haze-bot /haze-bot

FROM debian:trixie-slim
# yt-dlp en binario (asi `yt-dlp -U` se actualiza solo) y deno, que yt-dlp usa para
# resolver lo que YouTube esconde en JavaScript.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl unzip libopus0 \
    && curl -fsSL -o /usr/local/bin/yt-dlp https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux \
    && chmod +x /usr/local/bin/yt-dlp \
    && curl -fsSL -o /tmp/deno.zip https://github.com/denoland/deno/releases/latest/download/deno-x86_64-unknown-linux-gnu.zip \
    && unzip /tmp/deno.zip -d /usr/local/bin && rm /tmp/deno.zip \
    && apt-get purge -y unzip && apt-get autoremove -y && rm -rf /var/lib/apt/lists/*
COPY --from=build /haze-bot /usr/local/bin/haze-bot
CMD ["haze-bot"]
