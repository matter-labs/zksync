FROM alpine
EXPOSE 9876
ENV RUST_LOG info
COPY target/x86_64-unknown-linux-musl/release/dev-ticker-server /bin/
ENTRYPOINT ["dev-ticker-server"]
