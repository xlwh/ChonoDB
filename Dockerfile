# ChronoDB Dockerfile
# 构建阶段
FROM rust:1.75-slim AS builder

WORKDIR /app

# 安装构建依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# 复制 Cargo 文件
COPY Cargo.toml Cargo.lock ./
COPY storage/Cargo.toml storage/
COPY server/Cargo.toml server/
COPY chronodb-cli/Cargo.toml chronodb-cli/

# 创建虚拟 main.rs 以缓存依赖
RUN mkdir -p storage/src server/src chronodb-cli/src && \
    echo "fn main() {}" > storage/src/lib.rs && \
    echo "fn main() {}" > server/src/main.rs && \
    echo "fn main() {}" > chronodb-cli/src/main.rs && \
    cargo build --release && \
    rm -rf storage/src server/src chronodb-cli/src

# 复制源代码
COPY storage/src storage/src/
COPY server/src server/src/
COPY chronodb-cli/src chronodb-cli/src/

# 构建
RUN cargo build --release

# 运行阶段
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# 创建用户和目录
RUN groupadd -r chronodb && useradd -r -g chronodb chronodb && \
    mkdir -p /var/lib/chronodb /var/log/chronodb /etc/chronodb /etc/chronodb/rules /etc/chronodb/targets && \
    chown -R chronodb:chronodb /var/lib/chronodb /var/log/chronodb

# 复制二进制文件
COPY --from=builder /app/target/release/chronodb-server /usr/local/bin/chronodb-server
COPY --from=builder /app/target/release/chronodb /usr/local/bin/chronodb

# 复制默认配置
COPY config/chronodb.yaml /etc/chronodb/

# 复制监控配置
COPY config/prometheus.yml /etc/chronodb/
COPY config/alertmanager.yml /etc/chronodb/

# 复制规则和目标配置
COPY config/rules/ /etc/chronodb/rules/
COPY config/targets/ /etc/chronodb/targets/

# 设置权限
RUN chmod +x /usr/local/bin/chronodb-server /usr/local/bin/chronodb && \
    chown -R chronodb:chronodb /etc/chronodb

# 切换到非 root 用户
USER chronodb

# 暴露端口
EXPOSE 9090 9091

# 数据卷
VOLUME ["/var/lib/chronodb", "/var/log/chronodb"]

# 健康检查
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:9090/-/healthy || exit 1

# 启动命令
ENTRYPOINT ["chronodb-server"]
CMD ["--config.file=/etc/chronodb/chronodb.yaml"]
