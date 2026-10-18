FROM rust:1-alpine3.24 AS builder

RUN apk add --no-cache musl-dev cmake make

# audiopus_sys builds its bundled Opus with `cmake_minimum_required(VERSION 3.1)`, which
# CMake 4 rejects unless told to treat it as 3.5.
ENV CMAKE_POLICY_VERSION_MINIMUM=3.5

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN --mount=type=cache,sharing=private,target=/app/target \
    --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release \
    && cp /app/target/release/discobot-rs /usr/local/bin/discobot


FROM alpine:3.24 AS runner

# yt-dlp has to keep up with YouTube, so install the latest release from PyPI when the image
# is built rather than Alpine's package. Its "default" extra includes yt-dlp-ejs, which solves
# YouTube's JavaScript challenges using Deno. ffmpeg isn't needed: songbird decodes audio itself.
# Alpine's Python ships without ensurepip, so the venv is created without pip and filled using
# Alpine's pip, which is removed again afterwards.
RUN apk add --no-cache deno python3 \
    && apk add --no-cache --virtual .pip py3-pip \
    && python3 -m venv --without-pip /opt/yt-dlp \
    && pip --python /opt/yt-dlp/bin/python install --no-cache-dir "yt-dlp[default]" \
    && apk del .pip \
    && ln -s /opt/yt-dlp/bin/yt-dlp /usr/local/bin/yt-dlp

COPY --from=builder /usr/local/bin/discobot /usr/local/bin/discobot

CMD ["/usr/local/bin/discobot"]
