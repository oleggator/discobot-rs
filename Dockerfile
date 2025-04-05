FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev cmake make

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN --mount=type=cache,sharing=private,target=/app/target \
    --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release \
    && cp /app/target/release/discobot-rs /usr/local/bin/discobot


FROM alpine:latest AS runner

RUN apk add --no-cache yt-dlp ffmpeg

COPY --from=builder /usr/local/bin/discobot /usr/local/bin/discobot

CMD ["/usr/local/bin/discobot"]
