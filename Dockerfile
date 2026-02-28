FROM rust:1.86-slim AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --locked

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /work

COPY --from=builder /app/target/release/trex /usr/local/bin/trex

EXPOSE 8080

ENTRYPOINT ["trex"]
CMD ["serve", "--host", "0.0.0.0", "--port", "8080"]
