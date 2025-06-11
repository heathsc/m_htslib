use std::num::ParseIntError;

use thiserror::Error;

use crate::{AuxError, BgzfError, CigarError, CramError, FaidxError, KHashError, KStringError, SamError};

#[derive(Error, Debug)]
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
    #[error("Aux Error: {0}")]
    AuxError(#[from] AuxError),
    #[error("Sam Error: {0}")]
    SamError(#[from] SamError),
    #[error("Bgzf Error: {0}")]
    BgzfError(#[from] BgzfError),
    #[error("Cram Error: {0}")]
    CramError(#[from] CramError),
    #[error("Cigar Error: {0}")]
    CigarError(#[from] CigarError),
    #[error("KHash Error: {0}")]
    KHashError(#[from] KHashError),
    #[error("KString Error: {0}")]
    KStringError(#[from] KStringError),
    #[error("Faidx Error: {0}")]
    FaidxError(#[from] FaidxError),   
    #[error("Parse Int Error: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error("Illegal Tid: {0}")]
    TidError(libc::c_int),
}
