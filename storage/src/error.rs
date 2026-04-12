use thiserror::Error;
use tokio::task::JoinError;
use walkdir::Error as WalkdirError;
use std::path::StripPrefixError;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Compression error: {0}")]
    Compression(String),

    #[error("Compression error: {0}")]
    CompressionError(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Series not found: {0}")]
    SeriesNotFound(u64),

    #[error("Label not found: {0}")]
    LabelNotFound(String),

    #[error("WAL error: {0}")]
    Wal(String),

    #[error("Index error: {0}")]
    Index(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Config error: {0}")]
    ConfigError(String),

    #[error("Storage full")]
    StorageFull,

    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(i64),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Overflow error: {0}")]
    Overflow(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("Walkdir error: {0}")]
    Walkdir(#[from] WalkdirError),

    #[error("Path error: {0}")]
    Path(#[from] StripPrefixError),
}

impl From<Box<bincode::ErrorKind>> for Error {
    fn from(e: Box<bincode::ErrorKind>) -> Self {
        Error::Internal(format!("Bincode error: {}", e))
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
        assert!(err.to_string().contains("IO error"));
    }

    #[test]
    fn test_error_serialization() {
        let json_err = serde_json::from_str::<i32>("not a number").unwrap_err();
        let err: Error = json_err.into();
        assert!(matches!(err, Error::Serialization(_)));
    }

    #[test]
    fn test_error_variants() {
        let err = Error::Compression("lz4 failed".to_string());
        assert!(err.to_string().contains("Compression error"));

        let err = Error::InvalidData("bad format".to_string());
        assert!(err.to_string().contains("Invalid data"));

        let err = Error::SeriesNotFound(42);
        assert!(err.to_string().contains("42"));

        let err = Error::LabelNotFound("job".to_string());
        assert!(err.to_string().contains("job"));

        let err = Error::Wal("write failed".to_string());
        assert!(err.to_string().contains("WAL error"));

        let err = Error::Index("bloom filter".to_string());
        assert!(err.to_string().contains("Index error"));

        let err = Error::Config("invalid".to_string());
        assert!(err.to_string().contains("Configuration error"));

        let err = Error::StorageFull;
        assert!(err.to_string().contains("Storage full"));

        let err = Error::InvalidTimestamp(-1);
        assert!(err.to_string().contains("-1"));

        let err = Error::Overflow("delta".to_string());
        assert!(err.to_string().contains("Overflow"));

        let err = Error::NotImplemented("feature".to_string());
        assert!(err.to_string().contains("Not implemented"));

        let err = Error::Internal("unexpected".to_string());
        assert!(err.to_string().contains("Internal error"));
    }

    #[test]
    fn test_error_display() {
        let err = Error::NotFound("block".to_string());
        assert_eq!(format!("{}", err), "Not found: block");

        let err = Error::Storage("disk full".to_string());
        assert_eq!(format!("{}", err), "Storage error: disk full");

        let err = Error::SerializationError("custom".to_string());
        assert!(err.to_string().contains("Serialization error"));
    }

    #[test]
    fn test_result_ok() {
        let res: Result<i32> = Ok(42);
        assert_eq!(res.unwrap(), 42);
    }

    #[test]
    fn test_result_err() {
        let res: Result<i32> = Err(Error::StorageFull);
        assert!(res.is_err());
    }
}
