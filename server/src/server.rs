use axum::serve;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::{self, Certificate, PrivateKey};
use std::fs::File;
use std::io::BufReader;
use tracing::info;

use crate::api::create_routes;
use crate::config::ServerConfig;
use crate::state::ServerState;
use crate::Result;

pub struct Server {
    config: ServerConfig,
    state: Arc<ServerState>,
}

impl Server {
    pub async fn new() -> Result<Self> {
        let config = ServerConfig::default();
        let state = ServerState::new(config.clone()).await?;

        Ok(Self { config, state })
    }

    pub async fn with_config(config: ServerConfig) -> Result<Self> {
        let state = ServerState::new(config.clone()).await?;

        Ok(Self { config, state })
    }

    pub async fn run(self) -> Result<()> {
        let addr = format!("{}:{}", self.config.listen_address, self.config.port);
        let socket_addr: SocketAddr = addr.parse()
            .map_err(|e| crate::error::ServerError::Config(format!("Invalid address: {}", e)))?;

        info!("Starting ChronoDB server on {}", addr);

        // 创建路由
        let app = create_routes(self.state);

        // 绑定地址
        let listener = TcpListener::bind(&socket_addr).await
            .map_err(|e| crate::error::ServerError::Internal(format!("Failed to bind: {}", e)))?;

        // 检查是否启用 TLS
        if self.config.tls.enabled {
            info!("TLS is enabled");
            run_tls_server(listener, app, &self.config.tls, self.config.port).await?;
        } else {
            info!("ChronoDB server listening on {} (HTTP)", addr);
            serve(listener, app)
                .await
                .map_err(|e| crate::error::ServerError::Internal(format!("Server error: {}", e)))?;
        }

        Ok(())
    }

    /// 获取服务器配置
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// 获取服务器状态
    pub fn state(&self) -> Arc<ServerState> {
        self.state.clone()
    }
}

/// 运行 TLS 服务器
async fn run_tls_server(
    listener: TcpListener,
    _app: axum::Router,
    tls_config: &crate::config::TlsConfig,
    port: u16,
) -> Result<()> {
    // 加载证书
    let cert_file = tls_config.cert_file.as_ref()
        .ok_or_else(|| crate::error::ServerError::Config("TLS cert_file not specified".to_string()))?;
    let key_file = tls_config.key_file.as_ref()
        .ok_or_else(|| crate::error::ServerError::Config("TLS key_file not specified".to_string()))?;

    let certs = load_certs(cert_file)?;
    let key = load_private_key(key_file)?;

    let config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| crate::error::ServerError::Config(format!("TLS config error: {}", e)))?;

    let acceptor = TlsAcceptor::from(Arc::new(config));

    info!("ChronoDB server listening on {} (HTTPS)", port);

    // 使用 tokio-rustls 处理 TLS 连接
    loop {
        let (stream, peer_addr) = listener.accept().await
            .map_err(|e| crate::error::ServerError::Internal(format!("Accept error: {}", e)))?;

        let acceptor = acceptor.clone();

        tokio::spawn(async move {
            match acceptor.accept(stream).await {
                Ok(_tls_stream) => {
                    // 这里需要适配 axum 的 serve 来处理 TLS 流
                    // 简化实现，实际项目中需要更完整的处理
                    info!("TLS connection from {}", peer_addr);
                }
                Err(e) => {
                    tracing::error!("TLS accept error from {}: {}", peer_addr, e);
                }
            }
        });
    }
}

/// 加载证书文件
fn load_certs(path: &str) -> Result<Vec<Certificate>> {
    let file = File::open(path)
        .map_err(|e| crate::error::ServerError::Config(format!("Failed to open cert file: {}", e)))?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader)
        .map_err(|e| crate::error::ServerError::Config(format!("Failed to parse certs: {}", e)))?;

    Ok(certs.into_iter().map(Certificate).collect())
}

/// 加载私钥文件
fn load_private_key(path: &str) -> Result<PrivateKey> {
    let file = File::open(path)
        .map_err(|e| crate::error::ServerError::Config(format!("Failed to open key file: {}", e)))?;
    let mut reader = BufReader::new(file);

    // 尝试加载 PKCS8 格式的密钥
    if let Ok(keys) = rustls_pemfile::pkcs8_private_keys(&mut reader) {
        if let Some(key) = keys.into_iter().next() {
            return Ok(PrivateKey(key));
        }
    }

    // 尝试加载 RSA 格式的密钥
    let file = File::open(path)
        .map_err(|e| crate::error::ServerError::Config(format!("Failed to open key file: {}", e)))?;
    let mut reader = BufReader::new(file);

    if let Ok(keys) = rustls_pemfile::rsa_private_keys(&mut reader) {
        if let Some(key) = keys.into_iter().next() {
            return Ok(PrivateKey(key));
        }
    }

    Err(crate::error::ServerError::Config("No private key found".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let config = ServerConfig::default();
        let server = Server::with_config(config).await.unwrap();

        assert_eq!(server.config().port, 9090);
        assert_eq!(server.config().listen_address, "0.0.0.0");
    }
}
