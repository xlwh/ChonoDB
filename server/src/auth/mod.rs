use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};

/// JWT 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    /// JWT 密钥
    pub secret: String,
    /// Token 过期时间（小时）
    pub expiration_hours: i64,
    /// 签发者
    pub issuer: String,
    /// 受众
    pub audience: String,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: "chronodb-secret-key-change-in-production".to_string(),
            expiration_hours: 24,
            issuer: "chronodb".to_string(),
            audience: "chronodb-api".to_string(),
        }
    }
}

/// JWT Claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// 用户 ID
    pub sub: String,
    /// 用户名
    pub username: String,
    /// 角色
    pub roles: Vec<String>,
    /// 签发时间
    pub iat: i64,
    /// 过期时间
    pub exp: i64,
    /// 签发者
    pub iss: String,
    /// 受众
    pub aud: String,
}

/// 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    pub roles: Vec<String>,
    pub enabled: bool,
}

/// 角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    Admin,
    Write,
    Read,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Admin => write!(f, "admin"),
            Role::Write => write!(f, "write"),
            Role::Read => write!(f, "read"),
        }
    }
}

impl std::str::FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "admin" => Ok(Role::Admin),
            "write" => Ok(Role::Write),
            "read" => Ok(Role::Read),
            _ => Err(format!("Unknown role: {}", s)),
        }
    }
}

/// 登录请求
#[derive(Debug, Clone, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// 登录响应
#[derive(Debug, Clone, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: UserInfo,
}

/// 用户信息（用于响应）
#[derive(Debug, Clone, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub roles: Vec<String>,
}

/// 认证错误
#[derive(Debug, Clone, Serialize)]
pub struct AuthError {
    pub error: String,
    pub message: String,
}

/// 认证管理器
#[derive(Debug, Clone)]
pub struct AuthManager {
    config: JwtConfig,
    users: Arc<std::sync::RwLock<std::collections::HashMap<String, User>>>,
}

impl AuthManager {
    pub fn new(config: JwtConfig) -> Self {
        let mut users = std::collections::HashMap::new();
        
        // 添加默认管理员用户
        users.insert(
            "admin".to_string(),
            User {
                id: "1".to_string(),
                username: "admin".to_string(),
                email: "admin@chronodb.local".to_string(),
                roles: vec!["admin".to_string()],
                enabled: true,
            },
        );
        
        // 添加默认只读用户
        users.insert(
            "readonly".to_string(),
            User {
                id: "2".to_string(),
                username: "readonly".to_string(),
                email: "readonly@chronodb.local".to_string(),
                roles: vec!["read".to_string()],
                enabled: true,
            },
        );

        Self {
            config,
            users: Arc::new(std::sync::RwLock::new(users)),
        }
    }

    /// 用户登录
    pub fn login(&self, username: &str, password: &str) -> Result<LoginResponse, AuthError> {
        // 验证用户（简化实现，实际应该验证密码哈希）
        let users = self.users.read().unwrap();
        let user = users.get(username).ok_or(AuthError {
            error: "invalid_credentials".to_string(),
            message: "Invalid username or password".to_string(),
        })?;

        if !user.enabled {
            return Err(AuthError {
                error: "user_disabled".to_string(),
                message: "User account is disabled".to_string(),
            });
        }

        // 验证密码（简化实现，实际应该使用密码哈希）
        if password != "password" {
            return Err(AuthError {
                error: "invalid_credentials".to_string(),
                message: "Invalid username or password".to_string(),
            });
        }

        // 生成 JWT Token
        let token = self.generate_token(user)?;

        info!("User logged in: {}", username);

        Ok(LoginResponse {
            token,
            token_type: "Bearer".to_string(),
            expires_in: self.config.expiration_hours * 3600,
            user: UserInfo {
                id: user.id.clone(),
                username: user.username.clone(),
                roles: user.roles.clone(),
            },
        })
    }

    /// 验证 Token
    pub fn verify_token(&self, token: &str) -> Result<Claims, AuthError> {
        let mut validation = Validation::new(Algorithm::HS256);
        // 设置受众验证
        validation.set_audience(&[&self.config.audience]);
        validation.set_issuer(&[&self.config.issuer]);
        
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.config.secret.as_bytes()),
            &validation,
        )
        .map_err(|e| AuthError {
            error: "invalid_token".to_string(),
            message: format!("Token validation failed: {}", e),
        })?;

        Ok(token_data.claims)
    }

    /// 生成 Token
    fn generate_token(&self, user: &User) -> Result<String, AuthError> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.config.expiration_hours);

        let claims = Claims {
            sub: user.id.clone(),
            username: user.username.clone(),
            roles: user.roles.clone(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
        };

        encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(self.config.secret.as_bytes()),
        )
        .map_err(|e| AuthError {
            error: "token_generation_failed".to_string(),
            message: format!("Failed to generate token: {}", e),
        })
    }

    /// 检查用户是否有权限
    pub fn check_permission(&self, claims: &Claims, required_role: Role) -> bool {
        let required_role_str = required_role.to_string();
        
        // Admin 拥有所有权限
        if claims.roles.contains(&"admin".to_string()) {
            return true;
        }

        // Write 角色可以读和写
        if required_role == Role::Read && claims.roles.contains(&"write".to_string()) {
            return true;
        }

        claims.roles.contains(&required_role_str)
    }

    /// 添加用户
    pub fn add_user(&self, user: User) -> Result<(), AuthError> {
        let mut users = self.users.write().unwrap();
        
        if users.contains_key(&user.username) {
            return Err(AuthError {
                error: "user_exists".to_string(),
                message: "User already exists".to_string(),
            });
        }

        let username = user.username.clone();
        users.insert(username.clone(), user);
        info!("User added: {}", username);
        
        Ok(())
    }

    /// 删除用户
    pub fn remove_user(&self, username: &str) -> Result<(), AuthError> {
        let mut users = self.users.write().unwrap();
        
        if !users.contains_key(username) {
            return Err(AuthError {
                error: "user_not_found".to_string(),
                message: "User not found".to_string(),
            });
        }

        users.remove(username);
        info!("User removed: {}", username);
        
        Ok(())
    }

    /// 获取所有用户
    pub fn get_users(&self) -> Vec<User> {
        let users = self.users.read().unwrap();
        users.values().cloned().collect()
    }
}

/// 认证中间件
pub async fn auth_middleware(
    State(auth_manager): State<Arc<AuthManager>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // 从请求头中获取 Authorization
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());

    let token = match auth_header {
        Some(value) if value.starts_with("Bearer ") => &value[7..],
        _ => {
            warn!("Missing or invalid Authorization header");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // 验证 Token
    match auth_manager.verify_token(token) {
        Ok(claims) => {
            // 将 claims 添加到请求扩展中
            request.extensions_mut().insert(claims);
            Ok(next.run(request).await)
        }
        Err(e) => {
            warn!("Token validation failed: {:?}", e);
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// 权限检查中间件
pub fn require_role(role: Role) -> impl Fn(axum::extract::Extension<Claims>) -> Result<(), StatusCode> {
    move |axum::extract::Extension(claims)| {
        let required_role_str = role.to_string();
        
        // Admin 拥有所有权限
        if claims.roles.contains(&"admin".to_string()) {
            return Ok(());
        }

        // Write 角色可以读和写
        if role == Role::Read && claims.roles.contains(&"write".to_string()) {
            return Ok(());
        }

        if claims.roles.contains(&required_role_str) {
            Ok(())
        } else {
            warn!(
                "User {} does not have required role: {}",
                claims.username, required_role_str
            );
            Err(StatusCode::FORBIDDEN)
        }
    }
}

/// TLS 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub enabled: bool,
    pub cert_path: String,
    pub key_path: String,
    pub ca_path: Option<String>,
    pub client_auth: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            cert_path: "/etc/chronodb/server.crt".to_string(),
            key_path: "/etc/chronodb/server.key".to_string(),
            ca_path: None,
            client_auth: false,
        }
    }
}

/// 认证配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub enabled: bool,
    pub jwt: JwtConfig,
    pub tls: TlsConfig,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            jwt: JwtConfig::default(),
            tls: TlsConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_config_default() {
        let config = JwtConfig::default();
        assert_eq!(config.expiration_hours, 24);
        assert_eq!(config.issuer, "chronodb");
        assert_eq!(config.audience, "chronodb-api");
    }

    #[test]
    fn test_role_display() {
        assert_eq!(Role::Admin.to_string(), "admin");
        assert_eq!(Role::Write.to_string(), "write");
        assert_eq!(Role::Read.to_string(), "read");
    }

    #[test]
    fn test_auth_manager_creation() {
        let auth_manager = AuthManager::new(JwtConfig::default());
        let users = auth_manager.get_users();
        assert_eq!(users.len(), 2);
    }

    #[test]
    fn test_login_success() {
        let auth_manager = AuthManager::new(JwtConfig::default());
        let result = auth_manager.login("admin", "password");
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert_eq!(response.token_type, "Bearer");
        assert_eq!(response.user.username, "admin");
    }

    #[test]
    fn test_login_failure() {
        let auth_manager = AuthManager::new(JwtConfig::default());
        let result = auth_manager.login("admin", "wrong_password");
        assert!(result.is_err());
    }

    #[test]
    fn test_token_verification() {
        let auth_manager = AuthManager::new(JwtConfig::default());
        let login_response = auth_manager.login("admin", "password").unwrap();
        
        let claims = auth_manager.verify_token(&login_response.token);
        if let Err(ref e) = claims {
            eprintln!("Token verification error: {:?}", e);
        }
        assert!(claims.is_ok(), "Token verification failed: {:?}", claims);
        
        let claims = claims.unwrap();
        assert_eq!(claims.username, "admin");
        assert!(claims.roles.contains(&"admin".to_string()));
    }

    #[test]
    fn test_check_permission() {
        let auth_manager = AuthManager::new(JwtConfig::default());
        
        let admin_claims = Claims {
            sub: "1".to_string(),
            username: "admin".to_string(),
            roles: vec!["admin".to_string()],
            iat: 0,
            exp: 0,
            iss: "test".to_string(),
            aud: "test".to_string(),
        };
        
        let read_claims = Claims {
            sub: "2".to_string(),
            username: "readonly".to_string(),
            roles: vec!["read".to_string()],
            iat: 0,
            exp: 0,
            iss: "test".to_string(),
            aud: "test".to_string(),
        };

        // Admin 拥有所有权限
        assert!(auth_manager.check_permission(&admin_claims, Role::Admin));
        assert!(auth_manager.check_permission(&admin_claims, Role::Write));
        assert!(auth_manager.check_permission(&admin_claims, Role::Read));

        // Read 用户只有读权限
        assert!(!auth_manager.check_permission(&read_claims, Role::Admin));
        assert!(!auth_manager.check_permission(&read_claims, Role::Write));
        assert!(auth_manager.check_permission(&read_claims, Role::Read));
    }
}
