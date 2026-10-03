# Build environment for tinytext.
# Ubuntu 24.04 is used as the base so the resulting binary links against
# glibc 2.39 and GTK 4.14, and therefore runs on Ubuntu 24.04+, Debian 13+
# and rolling distros such as Arch/Omarchy.
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential ca-certificates curl pkg-config libgtk-4-dev \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl -sSf https://sh.rustup.rs | CARGO_HOME=/usr/local/cargo sh -s -- -y --profile minimal --default-toolchain stable \
    && CARGO_HOME=/usr/local/cargo cargo install cargo-about --locked --features cli \
    && rm -rf /usr/local/cargo/registry \
    && chmod -R a+rX /usr/local/rustup /usr/local/cargo

WORKDIR /src
