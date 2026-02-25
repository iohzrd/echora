# Stage 1: Build backend
FROM rust:latest AS builder

# Install mediasoup build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    python3 \
    python3-pip \
    build-essential \
    cmake \
    clang \
    && rm -rf /var/lib/apt/lists/* \
    && rustup component add rustfmt

WORKDIR /app
# Cache dependencies by building a dummy project first
COPY backend/Cargo.toml backend/Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release 2>/dev/null || true && rm -rf src && rm -f target/release/backend target/release/deps/backend-*
# Build the real project
COPY backend/src/ ./src/
COPY backend/migrations/ ./migrations/
RUN cargo build --release

# Stage 2: Runtime
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3t64 \
    curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/backend ./echocell
EXPOSE 3000
ENV RUST_LOG=info
CMD ["./echocell"]
