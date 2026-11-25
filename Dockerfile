# syntax=docker/dockerfile:1
#
# Castle's box in one image: the daemon (castled) serving the app at the same address.
# Built from this folder:   docker compose up -d --build     (see compose.yml)
#
# Three stages. The first builds the interface (app/dist), the second the daemon, and the
# last holds only what runs: the daemon, the built interface, and the few programs the
# daemon calls (bash for Security's audits, smartctl for drive health).

FROM node:22-bookworm-slim AS web
WORKDIR /src/app
COPY app/package.json app/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY app/index.html app/vite.config.js app/svelte.config.js app/jsconfig.json ./
COPY app/public ./public
COPY app/src ./src
# The Clones script guide reads its pages from here (app/src/lib/script-guide.js).
COPY app/clonewars/automation ./clonewars/automation
RUN npm run build

FROM rust:1-bookworm AS box
WORKDIR /src/castled
COPY castled/Cargo.toml castled/Cargo.lock ./
COPY castled/src ./src
COPY castled/audits ./audits
# The example script the daemon carries inside it (castled/src/clones.rs).
COPY app/clonewars/automation/examples /src/app/clonewars/automation/examples
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/castled/target \
    cargo build --release --locked && cp target/release/castled /castled

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 bash smartmontools tini \
    && rm -rf /var/lib/apt/lists/*
COPY --from=box /castled /usr/local/bin/castled
COPY --from=web /src/app/dist /opt/castle/web

# The vault is whatever is mounted at /vault. Inside a container the daemon has to listen
# on every address, or nothing outside the container could reach it; compose.yml decides
# which of the computer's addresses that port is published on.
#
# Every folder mounted under /drives is a place the vault can be moved to from the app
# (Settings > Storage), and /state remembers which one was chosen.
RUN mkdir -p /drives /state
ENV CASTLE_DATA_DIR=/vault \
    CASTLE_BIND=0.0.0.0:8787 \
    CASTLE_WEB_DIR=/opt/castle/web \
    CASTLE_PLACES=/drives \
    CASTLE_STATE_DIR=/state
VOLUME ["/vault", "/state"]
EXPOSE 8787
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
    CMD bash -c 'exec 3<>/dev/tcp/127.0.0.1/8787' || exit 1
# tini passes Ctrl+C and "docker stop" on, so the box shuts down the way it does anywhere.
ENTRYPOINT ["tini", "--", "castled"]
