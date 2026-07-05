FROM rust:1.86-bookworm AS builder

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM builder AS test

RUN cargo test --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home event-hub

COPY --from=builder /app/target/release/event-hub /usr/local/bin/event-hub

USER event-hub

ENV HTTP_ADDR=0.0.0.0:3000

EXPOSE 3000

ENTRYPOINT ["/usr/local/bin/event-hub"]
