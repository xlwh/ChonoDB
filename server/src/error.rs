use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Storage error: {0}")]
    Storage(#[from] chronodb_storage::Error),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("API error: {0}")]
    Api(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
    
    #[error("Rule error: {0}")]
    Rule(String),
    
    #[error("Target error: {0}")]
    Target(String),
}

pub type Result<T> = std::result::Result<T, ServerError>;
