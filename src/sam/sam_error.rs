use std::{num::ParseIntError, str::Utf8Error};

use thiserror::Error;

use crate::{AuxError, CigarError, CramError, KStringError};

#[derive(Error, Debug, PartialEq, Eq)]
pub enum SamError {
    #[error("Failed to read SAM/BAM/CRAM header")]
    FailedHeaderRead,
    #[error("Failed to write SAM/BAM/CRAM header")]
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
    #[error("Aux Error: {0}")]
    AuxError(#[from] AuxError),
    #[error("Cram Error: {0}")]
    CramError(#[from] CramError),
    #[error("Utf8 Error: {0}")]
    Utf8Error(#[from] Utf8Error),
    #[error("Parse Int Error: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error("Cigar Error: {0}")]
    CigarError(#[from] CigarError),
    #[error("KString Error: {0}")]
    KStringError(#[from] KStringError),
    #[error("Error setting query name for Bam Record")]
    SetQnameFailed,
    #[error("Error parsing Sam Record")]
    ParseSamRecordFailed,
    #[error("Empty query name")]
    EmptyQueryName,
    #[error("Query name too long")]
    QueryTooLong,
    #[error("Empty Flag Field")]
    EmptyFlagField,
    #[error("Empty Cigar Field")]
    EmptyCigarField,
    #[error("Bad Flag Format")]
    BadFlagFormat,
    #[error("Error parsing unsigned int")]
    ErrorParsingUint,
    #[error("Error parsing position")]
    ErrorParsingPos,
    #[error("No SQ lines found in header")]
    NoSqLines,
    #[error("Unrecognized reference name")]
    UnknownReference,
    #[error("Sequence line too long")]
    SeqTooLong,
    #[error("Too many Cigar elements")]
    TooManyCigarElem,
    #[error("Mismatch between Cigar and sequence length")]
    SeqCigarMismatch,
    #[error("Mismatch between quality and sequence length")]
    SeqQualMismatch,
}
