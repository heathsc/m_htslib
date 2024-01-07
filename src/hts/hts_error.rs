use thiserror::Error;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum HtsError {
    #[error("End of file")]
    EOF,
    #[error("Operation failed")]
    OperationFailed,
    #[error("Add Opt operation failed")]
    AddOptOperationFailed,
    #[error("Opt apply operation failed")]
    OptApplyOperationFailed,
    #[error("Parse Opt List operation failed")]
    ParseOptListOperationFailed,
    #[error("Parse Format operation failed")]
    ParseFormatOperationFailed,
    #[error("Seek failed")]
    SeekFailed,
    #[error("IO error")]
    IOError,
    #[error("Error opening file")]
    FileOpenError,
    #[error("Missing EOF marker")]
    MissingEOFMarker,
    #[error("No EOF marker for this filetype")]
    NoEOFMarkerForFileType,
    #[error("No EOF marker for this filetype")]
    NoEOFMarkerCheckForFileSystem,
    #[error("Unknown error")]
    UnknownError,
}
