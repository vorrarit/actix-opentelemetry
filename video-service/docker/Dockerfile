FROM ekidd/rust-musl-builder:1.57.0 as builder

WORKDIR /usr/src/myapp
COPY src src
COPY Cargo.toml Cargo.toml
RUN cargo build --release

# FROM debian:bullseye-slim
FROM alpine:latest
WORKDIR /app
COPY --from=builder /usr/src/myapp/target/x86_64-unknown-linux-musl/release/video-service video-service
EXPOSE 8080
CMD ["./video-service"]
