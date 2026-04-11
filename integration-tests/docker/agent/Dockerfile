# prom-agent Dockerfile
FROM prom/prometheus:v2.47.0

# 复制配置文件
COPY chronodb-agent.yaml /etc/prometheus/agent/chronodb-agent.yaml
COPY chronodb-node-1-agent.yaml /etc/prometheus/agent/chronodb-node-1-agent.yaml
COPY chronodb-node-2-agent.yaml /etc/prometheus/agent/chronodb-node-2-agent.yaml
COPY chronodb-node-3-agent.yaml /etc/prometheus/agent/chronodb-node-3-agent.yaml
COPY prometheus-agent.yaml /etc/prometheus/agent/prometheus-agent.yaml

# 设置工作目录
WORKDIR /etc/prometheus

# 启动命令 - 默认启动 chronodb-agent
CMD ["--config.file=/etc/prometheus/agent/chronodb-agent.yaml", "--storage.tsdb.path=/prometheus", "--web.console.libraries=/etc/prometheus/console_libraries", "--web.console.templates=/etc/prometheus/consoles", "--web.listen-address=:9090"]
