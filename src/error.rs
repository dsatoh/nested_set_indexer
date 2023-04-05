use thiserror::Error;
use std::io;

pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    RuntimeError(String),

    #[error("Parent node not found: {0}")]
    ParentNodeNotFoundError(String),

    #[error("Root node not found. Remove `\"parent\"` from root node or set it to `null`")]
    RootNodeNotFoundError(),

    #[error(transparent)]
    StdIoError(#[from] io::Error),

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[error(transparent)]
    CsvError(#[from] csv::Error),
}
