# syntax=docker/dockerfile:1.7
# Multi-stage build: rust builder → debian-slim runtime.
# Final image target: <30 MB.

FROM rust:1.85-slim AS builder
WORKDIR /build

# Cache deps first. The stub tree must cover every target declared in
# Cargo.toml — there is a [lib] as well as a [[bin]] — plus `web/index.html`
# and `assets/*.png`, which `src/web.rs` pulls in with include_str!/include_bytes!.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src web assets \
    && echo "fn main(){}" > src/main.rs \
    && : > src/lib.rs \
    && : > web/index.html \
    && : > assets/hermes-mark.png \
    && : > assets/hermes-favicon.png \
    && cargo build --release \
    && rm -rf src web assets target/release/deps/zero_hermes* target/release/deps/libzero_hermes*

COPY src ./src
COPY web ./web
COPY assets ./assets
COPY skills ./skills
RUN cargo build --release \
    && strip target/release/zero-hermes

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --shell /usr/sbin/nologin zh
COPY --from=builder /build/target/release/zero-hermes /usr/local/bin/zero-hermes
COPY --from=builder /build/skills /opt/zero-hermes/skills

ENV ZERO_HERMES_CONFIG=/config/zero_hermes.toml \
    ZERO_HERMES_SKILLS_DIR=/opt/zero-hermes/skills \
    RUST_LOG=zero_hermes=info

USER zh
WORKDIR /home/zh
VOLUME ["/config", "/data"]

ENTRYPOINT ["/usr/local/bin/zero-hermes"]
CMD ["gateway"]
