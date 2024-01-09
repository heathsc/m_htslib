use thiserror::Error;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum CramError {
    #[error("Error opening CRAM file")]
    OpenError,
    #[error("End of file")]
    EOF,
    #[error("IO Error")]
    IoError,
    #[error("Operation failed")]
    OperationFailed,
    #[error("Missing EOF marker")]
    MissingEOFMarker,
    #[error("Cannot check EOF on this file type")]
    CannotCheckEOF,
    #[error("Unknown error")]
    UnknownError,
}
