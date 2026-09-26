FROM rust:1.95-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/walletguard-api /usr/local/bin/walletguard-api
ENV PORT=3001
USER 65532:65532
EXPOSE 3001
CMD ["walletguard-api"]
