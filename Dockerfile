FROM rust:1.30.1

WORKDIR /usr/src/bellman
COPY . .

WORKDIR /usr/src/bellman/bellman-demo
RUN rustc -V
RUN cargo build --release
RUN cargo install --path .

ENV BELLMAN_VERBOSE=1
WORKDIR /
CMD ["bench"]
