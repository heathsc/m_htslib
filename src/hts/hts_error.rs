use std::convert::From;

use thiserror::Error;

use crate::{BgzfError, CigarError, CramError, KHashError, KStringError, SamError};

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
    #[error("Failed to initialize index")]
    IndexInitFailed,
    #[error("Invalid Index Format")]
    InvalidIndexFormat,
    #[error("Out of memory")]
    OutOfMemory,
    #[error("No stats information available")]
    StatsUnavailable,
    #[error("Sam Error: {0}")]
    SamError(SamError),
    #[error("Bgzf Error: {0}")]
    BgzfError(BgzfError),
    #[error("Cram Error: {0}")]
    CramError(CramError),
    #[error("Cigar Error: {0}")]
    CigarError(CigarError),
    #[error("KHash Error: {0}")]
    KHashError(KHashError),
    #[error("KString Error: {0}")]
    KStringError(KStringError),
}

impl From<SamError> for HtsError {
    fn from(value: SamError) -> Self {
        Self::SamError(value)
    }
}

impl From<BgzfError> for HtsError {
    fn from(value: BgzfError) -> Self {
        Self::BgzfError(value)
    }
}

impl From<CramError> for HtsError {
    fn from(value: CramError) -> Self {
        Self::CramError(value)
    }
}

impl From<CigarError> for HtsError {
    fn from(value: CigarError) -> Self {
        Self::CigarError(value)
    }
}

impl From<KHashError> for HtsError {
    fn from(value: KHashError) -> Self {
        Self::KHashError(value)
    }
}

impl From<KStringError> for HtsError {
    fn from(value: KStringError) -> Self {
        Self::KStringError(value)
    }
}
