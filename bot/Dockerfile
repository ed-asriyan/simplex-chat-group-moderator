FROM rust:1.95 AS dev
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src/ src/

FROM dev AS test
CMD cargo test --release

FROM dev AS builder
RUN cargo build --release

FROM debian:bookworm AS base
RUN apt update && apt install -y --no-install-recommends libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*

FROM base AS bot
COPY --from=builder /app/target/release/bot /bot

ENV SIMPLEX_WS_URI=ws://localhost:5225
ENV BOT_DISPLAY_NAME=test
ENV BOT_FULL_NAME=test
ENV BOT_SHORT_DESCRIPTION=test
ENV DB_PATH=/bot.db

CMD /bot \
    --display-name "$BOT_DISPLAY_NAME" \
    --full-name "$BOT_FULL_NAME" \
    --short-description "$BOT_SHORT_DESCRIPTION" \
    --simplex-uri "$SIMPLEX_WS_URI" \
    --db-path "$DB_PATH"
