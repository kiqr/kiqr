# KIQR — single image: daemon + web UI + CLI.
#
#   docker run -d -p 7777:7777 -v kiqr-data:/data --env-file .env ghcr.io/kiqr/kiqr
#
# Auth inside the container is by environment variables in .env:
#   ANTHROPIC_API_KEY / OPENAI_API_KEY, or CLAUDE_CODE_OAUTH_TOKEN from `claude setup-token`.

FROM node:22-bookworm-slim AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY skills ./skills
COPY seemo ./seemo
COPY --from=web /src/web/dist ./web/dist
RUN cargo build --release --locked -p kiqr

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates git curl \
 && rm -rf /var/lib/apt/lists/* \
 && mkdir -p /data && useradd -r -u 1000 -d /data kiqr && chown kiqr /data
# Claude Code CLI, for subscription auth via CLAUDE_CODE_OAUTH_TOKEN (optional at runtime).
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
 && apt-get install -y --no-install-recommends nodejs \
 && npm install -g @anthropic-ai/claude-code \
 && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/kiqr /usr/local/bin/kiqr
USER kiqr
ENV KIQR_HOME=/data KIQR_HOST=0.0.0.0
VOLUME ["/data"]
EXPOSE 7777
ENTRYPOINT ["kiqr"]
CMD ["daemon"]
