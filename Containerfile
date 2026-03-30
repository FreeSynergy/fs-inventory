# Containerfile — fs-inventory
# Multi-stage build: compile → minimal runtime image.

FROM rust:1.87-slim AS builder

WORKDIR /build

# Install protoc dependencies (needed for tonic-build / protoc-bin-vendored).
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies.
COPY Cargo.toml Cargo.lock build.rs ./
COPY proto/ ./proto/
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs && \
    echo 'pub fn dummy() {}' > src/lib.rs && \
    cargo build --release 2>/dev/null || true

# Full build.
COPY src/ ./src/
COPY ../fs-libs/ ../fs-libs/
COPY ../fs-db/ ../fs-db/
COPY ../fs-bus/ ../fs-bus/
COPY ../fs-i18n/ ../fs-i18n/
RUN cargo build --release

# ── Runtime image ──────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/fs-inventory /usr/local/bin/fs-inventory
COPY --from=builder /build/../fs-i18n/locales /usr/share/freesynergy/locales

# Data directory for the SQLite database.
RUN mkdir -p /var/lib/freesynergy

ENV FS_INVENTORY_DB=/var/lib/freesynergy/inventory.db
ENV FS_GRPC_PORT=50052
ENV FS_REST_PORT=8082
ENV FS_LOCALES_DIR=/usr/share/freesynergy/locales

EXPOSE 50052 8082

VOLUME ["/var/lib/freesynergy"]

ENTRYPOINT ["fs-inventory"]
CMD ["serve"]
