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
}
