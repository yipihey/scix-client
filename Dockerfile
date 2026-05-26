# Multi-stage build for the scix CLI / MCP server.
#
# Build:  docker build -t scix-client .
# Run:    docker run --rm -e SCIX_API_TOKEN=... ghcr.io/yipihey/scix-client serve

FROM rust:1.83-slim AS builder
WORKDIR /build

# Pre-cache deps for faster incremental builds.
COPY Cargo.toml ./
RUN mkdir src && \
    echo 'fn main() {}' > src/main.rs && \
    cargo fetch && \
    rm -rf src

COPY . .
RUN cargo build --release --features cli --bin scix && \
    strip target/release/scix

# --- Runtime stage: minimal Debian for HTTPS + glibc ---
FROM debian:bookworm-slim AS runtime
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/scix /usr/local/bin/scix

# Drop privileges.
RUN useradd --create-home --uid 1000 scix
USER scix
WORKDIR /home/scix

ENTRYPOINT ["/usr/local/bin/scix"]
CMD ["--help"]
