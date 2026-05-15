# T-meet — multi-stage build producing a single static binary in a scratch
# image. Total final image size: ~12 MB.
#
# Build:
#   docker build -t t-meet:1.0.0 .
#
# Run (manual):
#   docker run --rm -it -p 8443:8443 -p 8080:8080 \
#       -e MEET_ADMIN_PASSPHRASE='change me' \
#       -v meet-data:/opt/meet/data \
#       t-meet:1.0.0
#
# Preferred: use the docker-compose.yml at the repo root.

############################################################################
# Stage 1 — frontend build
############################################################################
FROM node:20-bookworm-slim AS frontend
WORKDIR /src
COPY frontend/package.json frontend/pnpm-lock.yaml frontend/scripts /src/frontend/
RUN corepack enable && corepack prepare pnpm@9.12.3 --activate \
    && pnpm -C /src/frontend install --frozen-lockfile
COPY frontend /src/frontend
RUN pnpm -C /src/frontend build

############################################################################
# Stage 2 — rust + musl build
############################################################################
FROM rust:1.83-bookworm AS backend

RUN apt-get update \
    && apt-get install -y --no-install-recommends musl-tools pkg-config \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add x86_64-unknown-linux-musl

WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY .cargo ./.cargo
COPY rustfmt.toml clippy.toml ./
COPY crates ./crates
COPY migrations ./migrations
# Pull in the frontend bundle so rust-embed picks it up at compile time.
COPY --from=frontend /src/frontend/dist /src/frontend/dist

ENV RUSTFLAGS="-C target-feature=+crt-static"
RUN cargo build --release --target x86_64-unknown-linux-musl -p meet-server

############################################################################
# Stage 3 — runtime
############################################################################
# Alpine because the compose entry needs a shell to branch on first-boot.
# `wget` ships in busybox so the healthcheck has zero extra deps.
FROM alpine:3.20

# Dedicated unprivileged user for the binary.
RUN addgroup -S -g 1000 meet \
    && adduser -S -D -u 1000 -G meet -h /opt/meet meet

WORKDIR /opt/meet

COPY --from=backend /src/target/x86_64-unknown-linux-musl/release/meet-server /opt/meet/meet-server
COPY config.example.toml /opt/meet/config.example.toml
COPY LICENSE /opt/meet/LICENSE

RUN chmod +x /opt/meet/meet-server \
    && mkdir -p /opt/meet/data \
    && chown -R meet:meet /opt/meet

USER meet:meet
VOLUME ["/opt/meet/data"]

EXPOSE 8443 8080

# meet-server reads ./config.toml from the working directory if present;
# operators bind-mount their own as documented in docker-compose.yml.
ENTRYPOINT ["/opt/meet/meet-server"]
CMD ["serve"]
