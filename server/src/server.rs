use axum::serve;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, error};

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
        
        info!("ChronoDB server listening on {}", addr);
        
        // 启动服务器
        serve(listener, app)
            .await
            .map_err(|e| crate::error::ServerError::Internal(format!("Server error: {}", e)))?;
        
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
