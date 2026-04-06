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
