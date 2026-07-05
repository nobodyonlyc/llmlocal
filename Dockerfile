# syntax=docker/dockerfile:1

FROM rust:1-slim-trixie AS builder
WORKDIR /build

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev protobuf-compiler cmake clang nasm ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --bin server

FROM debian:trixie-slim AS runtime
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 libgomp1 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/server /app/server

ENV SERVER_HOST=0.0.0.0
ENV FASTEMBED_CACHE_DIR=/data/fastembed_cache
VOLUME ["/data/fastembed_cache"]

EXPOSE 3000
ENTRYPOINT ["/app/server"]
