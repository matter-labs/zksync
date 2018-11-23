FROM rust:1.23.0

WORKDIR /usr/src/bellman
COPY . .

WORKDIR /usr/src/bellman/bellman-demo
RUN cargo build --release
RUN cargo install --path .

WORKDIR /
CMD ["bench"]
