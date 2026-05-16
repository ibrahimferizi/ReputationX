FROM rust:1.82-bookworm AS builder
WORKDIR /app
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/walletguard-api /usr/local/bin/walletguard-api
ENV PORT=3001
EXPOSE 3001
CMD ["walletguard-api"]
