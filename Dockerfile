# Templates (Askama) and migrations (sqlx::migrate!) are compiled into the
# binary at build time, so the runtime image only needs the binary + static/.
FROM rust:1-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
COPY templates ./templates
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/kyosabi /usr/local/bin/kyosabi
COPY static ./static

ENV DATABASE_PATH=/data/app.db
ENV APP_PORT=3000
EXPOSE 3000
VOLUME ["/data"]

ENTRYPOINT ["kyosabi"]
