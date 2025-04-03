FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev cmake make

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release

# Copy the actual source code
COPY src ./src

RUN cargo build --release


FROM alpine:latest AS runner

RUN apk add --no-cache yt-dlp ffmpeg

WORKDIR /app

COPY --from=builder /app/target/release/discobot-rs .

CMD ["./discobot-rs"]
