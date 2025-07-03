use std::{num::ParseIntError, str::Utf8Error};

use thiserror::Error;
use libc::c_int;

use crate::{AuxError, CigarError, CramError, FaidxError, KStringError, ParseINumError};

#[derive(Error, Debug)]
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
    #[error("Header text not parsed")]
    HeaderNotParsed,
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
    #[error("Faidx Error: {0}")]
    FaidxError(#[from] FaidxError),
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
    #[error("Bad characters in query name")]
    IllegalQueryNameChar,
    #[error("Empty query name")]
    EmptyQueryName,
    #[error("Query name too long")]
    QueryNameTooLong,
    #[error("Empty Flag Field")]
    EmptyFlagField,
    #[error("Empty Cigar Field")]
    EmptyCigarField,
    #[error("Cigar length not a multiple of 4")]
    CigarLengthNotMul4,
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
    #[error("Sequence length not set")]
    SeqLenNotSet,
    #[error("Sequence length mismatch")]
    SeqLenMismatch,
    #[error("Qual length does not match expectations")]
    UnexpectedQualLen,
    #[error("Not all mandatory fields are present in Bam record data segment")]
    IncompleteDataSegment,
    #[error("Can only use seq_writer when writing Seq section")]
    IllegalUseOfSeqWriter,
    #[error("Can only use cigar_writer when writing Cigar section")]
    IllegalUseOfCigarWriter,
    #[error("Can only use aux_writer when writing Aux section")]
    IllegalUseOfAuxWriter,
    #[error("Parse number error: {0}")]
    INumError(#[from] ParseINumError),
    #[error("Error reading from SAM/BAM/CRAM file: {0}")]
    SamReadError(c_int),
    #[error("Query region invalid: {0}")]
    InvalidRegion(String),
    #[error("BAQ realignment failed (out of memory)")]
    BaqRealignOutOfMem,
    #[error("BAQ realignment failed - nothing to realign")]
    BaqRealignFailed,
    #[error("BAQ realignment failed - unknown error")]
    BaqRealignUnknownError,
}
