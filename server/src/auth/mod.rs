use axum::{
    body::Body,
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use tracing::{debug, warn};

use crate::config::AuthConfig;
use crate::state::ServerState;

/// 认证中间件
pub async fn auth_middleware(
    state: axum::extract::State<Arc<ServerState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let config = &state.config.auth;
    
    // 如果认证未启用，直接放行
    if !config.enabled {
        return Ok(next.run(request).await);
    }
    
    // IP白名单检查
    if config.enable_ip_whitelist && !config.allowed_ips.is_empty() {
        let client_ip = request
            .headers()
            .get("x-forwarded-for")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string())
            .or_else(|| {
                request
                    .extensions()
                    .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
                    .map(|addr| addr.ip().to_string())
            })
            .unwrap_or_else(|| "unknown".to_string());
        
        if !config.allowed_ips.iter().any(|ip| client_ip.starts_with(ip)) {
            warn!("Access denied for IP: {}", client_ip);
            return Err(StatusCode::FORBIDDEN);
        }
    }
    
    // 根据认证类型进行认证
    match config.auth_type.as_str() {
        "basic" => authenticate_basic(request, next, config).await,
        "bearer" => authenticate_bearer(request, next, config).await,
        "api_key" => authenticate_api_key(request, next, config).await,
        _ => {
            warn!("Unknown auth type: {}", config.auth_type);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Basic 认证
async fn authenticate_basic(
    request: Request,
    next: Next,
    config: &AuthConfig,
) -> Result<Response, StatusCode> {
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());
    
    if let Some(auth_str) = auth_header {
        if auth_str.starts_with("Basic ") {
            let credentials = &auth_str[6..];
            let decoded = base64::decode(credentials).map_err(|_| StatusCode::UNAUTHORIZED)?;
            let decoded_str = String::from_utf8(decoded).map_err(|_| StatusCode::UNAUTHORIZED)?;
            
            let parts: Vec<&str> = decoded_str.splitn(2, ':').collect();
            if parts.len() == 2 {
                let username = parts[0];
                let password = parts[1];
                
                let config_username = config.username.as_deref().unwrap_or("");
                let config_password = config.password.as_deref().unwrap_or("");
                
                if username == config_username && password == config_password {
                    debug!("Basic authentication successful for user: {}", username);
                    return Ok(next.run(request).await);
                }
            }
        }
    }
    
    // 认证失败，返回 401
    let mut response = Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(Body::empty())
        .unwrap();
    
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        header::HeaderValue::from_static("Basic realm=\"ChronoDB\""),
    );
    
    warn!("Basic authentication failed");
    Ok(response)
}

/// Bearer Token 认证 (JWT)
async fn authenticate_bearer(
    request: Request,
    next: Next,
    config: &AuthConfig,
) -> Result<Response, StatusCode> {
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());
    
    if let Some(auth_str) = auth_header {
        if auth_str.starts_with("Bearer ") {
            let token = &auth_str[7..];
            
            // 简单的 JWT 验证（实际项目中应该使用 jwt 库）
            if validate_jwt(token, config) {
                debug!("Bearer authentication successful");
                return Ok(next.run(request).await);
            }
        }
    }
    
    warn!("Bearer authentication failed");
    Err(StatusCode::UNAUTHORIZED)
}

/// API Key 认证
async fn authenticate_api_key(
    request: Request,
    next: Next,
    config: &AuthConfig,
) -> Result<Response, StatusCode> {
    // 从 Header 中获取 API Key
    let api_key_from_header = request
        .headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok());
    
    // 从 Query 参数中获取 API Key
    let api_key_from_query = request
        .uri()
        .query()
        .and_then(|q| {
            q.split('&')
                .find(|p| p.starts_with("api_key="))
                .map(|p| &p[8..])
        });
    
    let api_key = api_key_from_header.or(api_key_from_query);
    
    if let Some(key) = api_key {
        if config.api_keys.iter().any(|k| k == key) {
            debug!("API key authentication successful");
            return Ok(next.run(request).await);
        }
    }
    
    warn!("API key authentication failed");
    Err(StatusCode::UNAUTHORIZED)
}

/// 简单的 JWT 验证（示例实现）
fn validate_jwt(token: &str, config: &AuthConfig) -> bool {
    // 在实际项目中，应该使用 jsonwebtoken 库进行完整的 JWT 验证
    // 这里只是一个简单的示例
    if let Some(secret) = &config.jwt_secret {
        // 验证 token 不为空且长度合理
        if !token.is_empty() && token.len() > 10 {
            // 这里应该进行真正的 JWT 签名验证
            // 为了示例，我们只是检查 token 格式
            return token.split('.').count() == 3;
        }
    }
    false
}

/// 生成 Basic 认证凭证
pub fn generate_basic_auth_header(username: &str, password: &str) -> String {
    let credentials = format!("{}:{}", username, password);
    let encoded = base64::encode(credentials);
    format!("Basic {}", encoded)
}

/// 生成 API Key
pub fn generate_api_key() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    const KEY_LEN: usize = 32;
    
    let mut rng = rand::thread_rng();
    let key: String = (0..KEY_LEN)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();
    
    format!("chronodb_{}", key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_basic_auth_header() {
        let header = generate_basic_auth_header("admin", "password123");
        assert!(header.starts_with("Basic "));
        assert_eq!(header.len(), 6 + 24); // "Basic " + base64("admin:password123")
    }

    #[test]
    fn test_generate_api_key() {
        let key1 = generate_api_key();
        let key2 = generate_api_key();
        
        assert!(key1.starts_with("chronodb_"));
        assert!(key2.starts_with("chronodb_"));
        assert_ne!(key1, key2);
        assert_eq!(key1.len(), 9 + 32); // "chronodb_" + 32 random chars
    }

    #[test]
    fn test_validate_jwt() {
        let config = AuthConfig {
            jwt_secret: Some("secret".to_string()),
            ..Default::default()
        };
        
        // 有效的 JWT 格式（三部分）
        assert!(validate_jwt("eyJhbGciOiJIUzI1NiIs.eyJzdWIiOiIxMjM0NTY3ODkwIiw.DTyz75KJGxG3MlY", &config));
        
        // 无效的 JWT 格式
        assert!(!validate_jwt("invalid", &config));
        assert!(!validate_jwt("", &config));
    }
}
