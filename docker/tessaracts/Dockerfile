FROM alpine:edge AS builder

RUN apk add build-base \
    cmake \
    linux-headers \
    openssl-dev \
    cargo \
    clang \
    clang-libs \
    git

RUN git clone https://github.com/adriamb/tesseracts.git
WORKDIR /
COPY . .
WORKDIR /tesseracts
RUN cargo build --release

FROM alpine:edge
WORKDIR /
COPY --from=builder /tesseracts.toml .
COPY --from=builder /tesseracts/target/release/tesseracts .

EXPOSE 8080

RUN apk add clang clang-libs ca-certificates

ENTRYPOINT ["./tesseracts","--cfg","tesseracts.toml","-vvv"]