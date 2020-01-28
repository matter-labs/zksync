FROM alpine
EXPOSE 3000
EXPOSE 3031
EXPOSE 3030
ARG BINARY[=target/x86_64-unknown-linux-musl/release/server]
COPY $BINARY /
ENTRYPOINT ["/server"]
