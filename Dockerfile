# 构建阶段
FROM rust:1.85-bookworm AS builder
WORKDIR /build

# 覆盖项目的 .cargo/config.toml（原配置默认 musl 目标，Docker 中用 glibc）
RUN mkdir -p .cargo && echo '' > .cargo/config.toml

# 依赖缓存层：先复制清单文件，构建空壳项目缓存依赖
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main(){}' > src/main.rs \
    && cargo build --release 2>/dev/null || true \
    && rm -rf src

# 复制源码并构建（再次覆盖 .cargo/config.toml 防止被 COPY 覆写）
COPY . .
RUN echo '' > .cargo/config.toml && touch src/main.rs && cargo build --release

# 运行阶段
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/cblog /usr/local/bin/cblog

WORKDIR /site
VOLUME ["/site"]
EXPOSE 3000

ENTRYPOINT ["cblog"]
CMD ["serve", "--host", "0.0.0.0"]
