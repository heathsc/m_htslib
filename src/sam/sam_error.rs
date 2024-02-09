use thiserror::Error;

#[derive(Error, Debug)]
pub enum SamError {
    #[error("Failed to write SAM header")]
    FailedHeaderWrite,
    #[error("Failed to add line to SAM/BAM/CRAM header")]
    FailedAddHeaderLine,
    #[error("Failed to remove lines from SAM/BAM/CRAM header")]
    FailedRemoveHeaderLines,
    #[error("Out of memory")]
    OutOfMemory,
    #[error("Failed to parse header text")]
    HeaderParseFailed,
    #[error("Operation failed")]
    OperationFailed,
    #[error("Illegal characters in new header line")]
    IllegalHeaderChars,
    #[error("Illegal Header Tag (Tag must be two characters)")]
    IllegalTagLength,
    #[error("Illegal Tag Value (interior null character)")]
    NullInTagValue,
    #[error("PG ID Tag already exists in SAM header")]
    PgIdTagExists,
    #[error("PG ID Tag referred to in PP tag does not exist in SAM header")]
    PpRefTagMissing,
}
