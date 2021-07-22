# syntax=docker/dockerfile:experimental
FROM rust:1.51 as builder
WORKDIR /usr/src/zksync
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release

FROM debian:buster-slim
RUN apt-get update && apt-get install -y libpq5 ca-certificates && rm -rf /var/lib/apt/lists/*
EXPOSE 3000
EXPOSE 3031
EXPOSE 3030
COPY --from=builder /usr/src/zksync/target/release/zksync_server /usr/bin
COPY contracts/artifacts/ /contracts/artifacts/
ENTRYPOINT ["zksync_server"]
