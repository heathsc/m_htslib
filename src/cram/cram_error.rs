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
    #[error("Seek operation failed")]
    SeekFailed,
    #[error("Missing EOF marker")]
    MissingEOFMarker,
    #[error("Stream is not seeakable - cannot check EOF block")]
    CannotCheckEOF,
    #[error("CRAM version does not contain EOF blocks")]
    CramVersionHasNoEOF,
    #[error("Unknown error")]
    UnknownError,
}
