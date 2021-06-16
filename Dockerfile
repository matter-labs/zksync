# syntax=docker/dockerfile:experimental
FROM rust:1.48 as builder
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install sccache
WORKDIR /usr/src/zksync
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/root/.cache/sccache \
    RUSTC_WRAPPER=/usr/local/cargo/bin/sccache \
    cargo build --release

FROM debian:buster-slim
RUN apt update
RUN apt install openssl -y
EXPOSE 9876
ENV RUST_LOG info
COPY --from=builder /usr/src/zksync/target/release/dev-ticker-server /bin/
ENTRYPOINT ["dev-ticker-server"]
