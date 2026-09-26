FROM node:24-bookworm-slim AS frontend
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

FROM rust:1.95-bookworm AS backend
WORKDIR /app/backend
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=backend /app/backend/target/release/walletguard-api /usr/local/bin/walletguard-api
COPY --from=frontend /app/frontend/dist /app/public
ENV PORT=3001 STATIC_DIR=/app/public RUST_LOG=walletguard_api=info
USER 65532:65532
EXPOSE 3001
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s CMD curl --fail --silent http://127.0.0.1:${PORT}/health || exit 1
CMD ["walletguard-api"]
