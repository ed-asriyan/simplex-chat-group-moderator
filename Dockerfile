# Every image of the project, one stage each, built from the repository root
# (.dockerignore lets in only what the stages copy). A compose file picks its
# stage with `target`, and BuildKit builds only the stages that one needs.
#   smp-client      the SimpleX CLI the bot talks to
#   smp-client-e2e  the same, with the clock of the end-to-end tests
#   dev             the toolchain and the sources, for the dev container
#   test            the unit tests
#   bot             the production bot
#   bot-e2e         the production bot with the clock of the end-to-end tests

FROM --platform=linux/x86_64 ubuntu:26.04 AS smp-client
RUN apt-get update && apt-get install -y netcat && rm -rf /var/lib/apt/lists/*
ADD --chmod=700 https://github.com/simplex-chat/simplex-chat/releases/download/v7.0.0/simplex-chat-ubuntu-22_04-x86_64 /usr/bin/simplex-chat
ENV PORT=5225
ENV DB=/opt/simplex-chat/db
CMD simplex-chat --yes-migrate --create-bot-display-name bot -p $PORT -d $DB

# The bot dates a message by the time its CLI received it, so the CLI reads the
# tests' clock too: libfaketime in front of it, reading /clock/now, an offset in
# seconds from the real time ("+3600") that the tests rewrite as they go.
FROM smp-client AS smp-client-e2e
RUN apt-get update && apt-get install -y --no-install-recommends faketime && rm -rf /var/lib/apt/lists/*
ENV LD_PRELOAD=/usr/lib/x86_64-linux-gnu/faketime/libfaketimeMT.so.1 \
    FAKETIME_TIMESTAMP_FILE=/clock/now \
    FAKETIME_NO_CACHE=1 \
    FAKETIME_DONT_FAKE_MONOTONIC=1

FROM rust:1.99-bookworm AS dev
WORKDIR /app/bot
COPY bot/Cargo.toml bot/Cargo.lock ./
COPY bot/src/ src/

FROM dev AS test
# A test checks the schema in the design document against the migrations.
COPY docs/design.md /app/docs/design.md
CMD cargo test --release

FROM dev AS builder
RUN cargo build --release

FROM debian:bookworm AS base
RUN apt update && apt install -y --no-install-recommends libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*

FROM base AS bot
COPY --from=builder /app/bot/target/release/bot /bot

ENV SIMPLEX_WS_URI=ws://localhost:5225
ENV BOT_DISPLAY_NAME=test
ENV BOT_FULL_NAME=test
ENV BOT_SHORT_DESCRIPTION=test
ENV DB_PATH=/bot.db
ENV WEBEDITOR_BASE_URL=https://example.com/webeditor

CMD /bot \
    --display-name "$BOT_DISPLAY_NAME" \
    --full-name "$BOT_FULL_NAME" \
    --short-description "$BOT_SHORT_DESCRIPTION" \
    --simplex-uri "$SIMPLEX_WS_URI" \
    --db-path "$DB_PATH" \
    --webeditor-base-url "$WEBEDITOR_BASE_URL"

# The production bot, unchanged, with the tests' clock (see smp-client-e2e).
FROM bot AS bot-e2e
RUN apt-get update && apt-get install -y --no-install-recommends faketime && rm -rf /var/lib/apt/lists/*
ENV LD_PRELOAD=/usr/lib/x86_64-linux-gnu/faketime/libfaketimeMT.so.1 \
    FAKETIME_TIMESTAMP_FILE=/clock/now \
    FAKETIME_NO_CACHE=1 \
    FAKETIME_DONT_FAKE_MONOTONIC=1
