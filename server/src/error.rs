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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_error_variants() {
        let err = ServerError::Config("invalid yaml".to_string());
        assert!(err.to_string().contains("Configuration error"));

        let err = ServerError::Api("bad request".to_string());
        assert!(err.to_string().contains("API error"));

        let err = ServerError::NotFound("resource".to_string());
        assert!(err.to_string().contains("Not found"));

        let err = ServerError::InvalidRequest("missing param".to_string());
        assert!(err.to_string().contains("Invalid request"));

        let err = ServerError::Internal("unexpected".to_string());
        assert!(err.to_string().contains("Internal error"));

        let err = ServerError::Rule("eval failed".to_string());
        assert!(err.to_string().contains("Rule error"));

        let err = ServerError::Target("scrape failed".to_string());
        assert!(err.to_string().contains("Target error"));
    }

    #[test]
    fn test_result_ok() {
        let res: Result<i32> = Ok(42);
        assert_eq!(res.unwrap(), 42);
    }

    #[test]
    fn test_result_err() {
        let res: Result<i32> = Err(ServerError::NotFound("test".to_string()));
        assert!(res.is_err());
    }

    #[test]
    fn test_storage_error_conversion() {
        let storage_err = chronodb_storage::Error::InvalidData("bad".to_string());
        let server_err: ServerError = storage_err.into();
        assert!(matches!(server_err, ServerError::Storage(_)));
    }
}
