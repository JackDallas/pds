# Stage 1: Build
FROM rust:1.88-bookworm AS builder

WORKDIR /build

# Copy workspace manifests and lockfile for dependency caching
COPY Cargo.toml Cargo.lock ./

# Copy all crate Cargo.toml files
COPY crates/dallaspds-core/Cargo.toml crates/dallaspds-core/Cargo.toml
COPY crates/dallaspds-crypto/Cargo.toml crates/dallaspds-crypto/Cargo.toml
COPY crates/dallaspds-repo/Cargo.toml crates/dallaspds-repo/Cargo.toml
COPY crates/dallaspds-storage-sqlite/Cargo.toml crates/dallaspds-storage-sqlite/Cargo.toml
COPY crates/dallaspds-storage-postgres/Cargo.toml crates/dallaspds-storage-postgres/Cargo.toml
COPY crates/dallaspds-blob-fs/Cargo.toml crates/dallaspds-blob-fs/Cargo.toml
COPY crates/dallaspds-blob-s3/Cargo.toml crates/dallaspds-blob-s3/Cargo.toml
COPY crates/dallaspds-identity/Cargo.toml crates/dallaspds-identity/Cargo.toml
COPY crates/dallaspds-server/Cargo.toml crates/dallaspds-server/Cargo.toml
COPY crates/dallaspds-single/Cargo.toml crates/dallaspds-single/Cargo.toml
COPY crates/dallaspds-multi/Cargo.toml crates/dallaspds-multi/Cargo.toml
COPY crates/dallaspds-test-utils/Cargo.toml crates/dallaspds-test-utils/Cargo.toml

# Create stub source files so cargo can resolve the workspace and cache deps
RUN mkdir -p crates/dallaspds-core/src && echo "" > crates/dallaspds-core/src/lib.rs && \
    mkdir -p crates/dallaspds-crypto/src && echo "" > crates/dallaspds-crypto/src/lib.rs && \
    mkdir -p crates/dallaspds-repo/src && echo "" > crates/dallaspds-repo/src/lib.rs && \
    mkdir -p crates/dallaspds-storage-sqlite/src && echo "" > crates/dallaspds-storage-sqlite/src/lib.rs && \
    mkdir -p crates/dallaspds-storage-postgres/src && echo "" > crates/dallaspds-storage-postgres/src/lib.rs && \
    mkdir -p crates/dallaspds-blob-fs/src && echo "" > crates/dallaspds-blob-fs/src/lib.rs && \
    mkdir -p crates/dallaspds-blob-s3/src && echo "" > crates/dallaspds-blob-s3/src/lib.rs && \
    mkdir -p crates/dallaspds-identity/src && echo "" > crates/dallaspds-identity/src/lib.rs && \
    mkdir -p crates/dallaspds-server/src && echo "" > crates/dallaspds-server/src/lib.rs && \
    mkdir -p crates/dallaspds-single/src && echo "fn main() {}" > crates/dallaspds-single/src/main.rs && \
    mkdir -p crates/dallaspds-multi/src && echo "fn main() {}" > crates/dallaspds-multi/src/main.rs && \
    mkdir -p crates/dallaspds-test-utils/src && echo "" > crates/dallaspds-test-utils/src/lib.rs

# Build dependencies only
RUN cargo build --release -p dallaspds-single 2>&1 || true

# Remove stub source files and fingerprints so real source triggers rebuild
RUN rm -rf crates/*/src && \
    find target/release/.fingerprint -name "dallaspds-*" -exec rm -rf {} + 2>/dev/null || true

# Copy real source code
COPY crates/ crates/

# Build the real binary
RUN cargo build --release -p dallaspds-single

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends libsqlite3-0 ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary
COPY --from=builder /build/target/release/dallaspds-single ./dallaspds-single

# Copy docker config
COPY config/docker.toml ./config/docker.toml

# Create data directory for SQLite DB and blobs
RUN mkdir -p /app/data

ENV CONFIG_PATH=config/docker.toml

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s \
    CMD curl -f http://localhost:3000/xrpc/_health || exit 1

CMD ["./dallaspds-single"]
