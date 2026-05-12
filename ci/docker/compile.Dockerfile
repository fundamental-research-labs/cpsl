FROM ubuntu:24.04

ENV CARGO_TERM_COLOR=always
ENV DEBIAN_FRONTEND=noninteractive
ENV PATH=/root/.cargo/bin:$PATH

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        git \
        libwebkit2gtk-4.1-dev \
        pkg-config \
        zstd \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain stable
