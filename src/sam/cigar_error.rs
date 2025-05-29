use thiserror::Error;

use crate::ParseINumError;
use super::cigar::CigarElem;

#[derive(Error, Debug)]
pub enum CigarError {
    // Hard clip ops can only be at the end of cigar strings
    #[error("Hard clip operation not at ends of CIGAR")]
    // Only hard clip ops can be between a soft clip and the ends of the cigar
    InteriorHardClip,
    #[error("Soft clip operation in interior of CIGAR")]
    InteriorSoftClip,
    #[error("Multiple adjacent hard clip ops")]
    MultipleAdjacentHardClips,
    #[error("Multiple adjacent soft clip ops")]
    MultipleAdjacentSoftClips,
    #[error("Unknown Operator")]
    UnknownOperator,
    #[error("Missing Length")]
    MissingLength,
    #[error("Error Parsing Length")]
    BadLength,
    #[error("Missing Operator")]
    MissingOperator,
    #[error("Trailing Garbage")]
    TrailingGarbage,
    #[error("CIGAR too short for trim operation")]
    CigarTooShortForTrim,
    #[error("CIGAR op length overflow")]
    CigarOpLenOverflow,
    #[error("CIGAR missing op length")]
    MissingOpLen,
    #[error("Parse number error: {0}")]
    INumError(#[from] ParseINumError),
}

#[derive(Error, Debug)]
pub enum CigarTrimError {
    #[error("CIGAR reference length less than trim amount")]
    CigarTooShort(Vec<CigarElem>),
}
