# syntax=docker/dockerfile:1

# --- Build Rust --- (tag d'origine : rust:1.99-slim-bookworm)
FROM docker.io/library/rust@sha256:ed2c95ca377d6571b124b05e75e01207973ded639dce7d5542eb7bcd797e32aa AS build
WORKDIR /app
# Dépendances d'abord : cette couche ne change que si Cargo.toml/Cargo.lock changent.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && touch src/lib.rs \
    && cargo fetch --locked && rm -rf src
COPY src ./src
RUN cargo build --release --locked --bin socialvideodownload

# --- ffmpeg statique --- (tag d'origine : mwader/static-ffmpeg:7.1.1)
FROM docker.io/mwader/static-ffmpeg@sha256:11a44711684c0b9f754c047dcd64235b8b52deab251bd0e0a86f22faa160749c AS ffmpeg

# --- yt-dlp autonome, release figée + sha256 publié par yt-dlp --- (tag d'origine : debian:bookworm-slim)
FROM docker.io/library/debian@sha256:99c0b25c46d0214d5a2091092e7908907dcaecd6a962cde0ac700acee222b6f3 AS ytdlp
ARG TARGETARCH
ARG YTDLP_VERSION=2026.10.26
ARG YTDLP_SHA256_AMD64=767ba9dce7052aa196f8b12652f433d76de128ba168966fa84b6394d5e6c6795
ARG YTDLP_SHA256_ARM64=e87e9533e89fe5889abaa21f5114dfe7756d6aa6219b83551add04d48b18e483
ARG YTDLP_DIR_SHA256_AMD64=f34053415136e55073dd64ba5326d161ecc23ce0e98c081d8752867b37809450
ARG YTDLP_DIR_SHA256_ARM64=74708e707e8fa8177c8347e58152337edf320149cd7b225defd453f7cde47822
# Deux variantes : /yt-dlp (un fichier, se met à jour seul : Pelican) et /yt-dlp-dir (dossier : image
# standard). Le dossier ne se décompresse pas à chaque lancement : ~0,4 s de moins par téléchargement
# (316-408 ms contre 776-1222 ms, mesuré le 06/11/2026), pour +59 Mo décompressés (+1,2 Mo à télécharger).
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl unzip \
    && rm -rf /var/lib/apt/lists/* \
    && case "${TARGETARCH}" in \
         amd64) asset=yt-dlp_linux;         sha="${YTDLP_SHA256_AMD64}"; dsha="${YTDLP_DIR_SHA256_AMD64}" ;; \
         arm64) asset=yt-dlp_linux_aarch64; sha="${YTDLP_SHA256_ARM64}"; dsha="${YTDLP_DIR_SHA256_ARM64}" ;; \
         *) echo "architecture non prise en charge : ${TARGETARCH}" >&2; exit 1 ;; \
       esac \
    && base="https://github.com/yt-dlp/yt-dlp/releases/download/${YTDLP_VERSION}" \
    && curl -fsSL -o /yt-dlp "${base}/${asset}" \
    && echo "${sha}  /yt-dlp" | sha256sum -c - \
    && chmod 0755 /yt-dlp \
    && curl -fsSL -o /yt-dlp.zip "${base}/${asset}.zip" \
    && echo "${dsha}  /yt-dlp.zip" | sha256sum -c - \
    && unzip -q /yt-dlp.zip -d /yt-dlp-dir && rm /yt-dlp.zip \
    && mv "/yt-dlp-dir/${asset}" /yt-dlp-dir/yt-dlp \
    && chmod -R a+rX /yt-dlp-dir && chmod 0755 /yt-dlp-dir/yt-dlp

# --- Base commune aux deux images --- (tag d'origine : debian:bookworm-slim)
FROM docker.io/library/debian@sha256:99c0b25c46d0214d5a2091092e7908907dcaecd6a962cde0ac700acee222b6f3 AS runtime
# upgrade : correctifs de sécurité parus après l'image figée (le scan Trivy de la CI refuse une faille HIGH corrigée).
RUN apt-get update && apt-get upgrade -y \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/socialvideodownload /usr/local/bin/socialvideodownload
COPY --from=ffmpeg /ffmpeg /usr/local/bin/ffmpeg
COPY --from=ffmpeg /ffprobe /usr/local/bin/ffprobe
# Le bot s'arrête proprement sur SIGINT (^C) et ne gère pas SIGTERM, le signal par défaut de docker stop.
STOPSIGNAL SIGINT

# --- Variante Pelican/Pterodactyl (docker build --target pelican) ---
# Wings monte le serveur sur /home/container et y lance l'image sous son propre UID.
FROM runtime AS pelican
COPY --from=ytdlp /yt-dlp /usr/local/bin/yt-dlp
RUN useradd --create-home --home-dir /home/container --shell /bin/sh container
COPY pelican/entrypoint.sh /entrypoint.sh
# Sans .env sous Pelican : l'entrypoint y lit VERSION (affichée par /help) quand l'egg ne la donne pas.
COPY .env.example /usr/local/share/socialvideodownload/env.example
# yt-dlp tourne depuis une copie du volume, qu'il peut mettre à jour ; il s'extrait dans TMPDIR
# à chaque lancement, et le /tmp de Wings est un tmpfs de 100 Mo par défaut.
ENV USER=container HOME=/home/container TMPDIR=/home/container/tmp \
    YTDLP_PATH=/home/container/bin/yt-dlp
USER container
WORKDIR /home/container
ENTRYPOINT ["/bin/sh", "/entrypoint.sh"]

# --- Image standard (cible par défaut, compose) ---
FROM runtime
COPY --from=ytdlp /yt-dlp-dir /opt/yt-dlp
RUN ln -s /opt/yt-dlp/yt-dlp /usr/local/bin/yt-dlp \
    && useradd --system --uid 10001 --no-create-home --shell /usr/sbin/nologin svd \
    && mkdir -p /data/downloads /data/logs /data/state && chown -R 10001:10001 /data
ENV YTDLP_PATH=/usr/local/bin/yt-dlp \
    FFMPEG_PATH=/usr/local/bin/ffmpeg \
    FFPROBE_PATH=/usr/local/bin/ffprobe \
    DOWNLOAD_DIR=/data/downloads \
    SETTINGS_FILE=/data/state/settings.json
USER 10001:10001
WORKDIR /data
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 CMD ["/usr/local/bin/socialvideodownload","--healthcheck"]
ENTRYPOINT ["/usr/local/bin/socialvideodownload"]
