FROM alpine
EXPOSE 3000
EXPOSE 3031
EXPOSE 3030
COPY target/x86_64-unknown-linux-musl/release/server /
COPY contracts/build/ /contracts/build/
ENTRYPOINT ["/server"]
