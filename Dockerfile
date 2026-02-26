# 构建阶段
FROM rust:1.83-bookworm AS builder
WORKDIR /build

RUN apt-get update && apt-get install -y liblua5.4-dev pkg-config && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main(){}' > src/main.rs && cargo build --release 2>/dev/null || true && rm -rf src

COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y liblua5.4-0 ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/cblog /usr/local/bin/cblog

WORKDIR /site
VOLUME ["/site"]
EXPOSE 3000

ENTRYPOINT ["cblog"]
CMD ["serve", "--host", "0.0.0.0"]
