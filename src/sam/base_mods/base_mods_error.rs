use thiserror::Error;

use crate::{AuxError, ParseINumError};

use super::CanonicalBase;

#[derive(Error, Debug)]
pub enum BaseModsError {
    #[error("Illegal canonical base: {0}")]
    IllegalCanonicalBase(u8),
    #[error("Bad modifier base: {0}")]
    BadModifierBase(u8),
    #[error("Modification input too short")]
    ShortInput,
    #[error("Mismatch between canonical base {0:?}{1} and modification code {2}")]
    ModifierMismatch(CanonicalBase, char, char),
    #[error("Unknown modification code: {0}")]
    UnknownModCode(char),
    #[error("Error parsing integer: {0}")]
    ParseInumError(#[from] ParseINumError),
    #[error("Error parsing Aux tag: {0}")]
    ParseAuxError(#[from] AuxError),
    #[error("Malformed MM tag - illegal strand")]
    MalformedMMTag,
    #[error("Count does not start with a comma")]
    MissingCommaBeforeCount,
    #[error("Parse error for MM count")]
    MMCountParseError,
    #[error("Mismatch between MM tag and sequence - reference to bases outside of sequence")]
    MMSeqMismatch,
    #[error("Bad implicit code in MM tag: {0}")]
    BadImplicitMMCode2(char),
    #[error("Bad implicit code in MM tag")]
    BadImplicitMMCode,
    #[error("Base count overflow")]
    BaseCountOverflow,
    #[error("Trailing garbage in modification description")]
    TrailingGarbageModDesc,
    #[error("MM tag is not a String type")]
    MMTagNotString,
    #[error("ML tag is not an UInt8 Array")]
    MLTagNotUInt8Array,
    #[error("MN tag is not an Integer type")]
    MNTagNotInteger,
    #[error("Multiple MM tags found")]
    MultipleMMTags,
    #[error("Multiple ML tags found")]
    MultipleMLTags,
    #[error("Multiple MN tags found")]
    MultipleMNTags,
    #[error("MM tag is empty")]
    EmptyMMTag,
    #[error("MM tag not terminated by semicolon")]
    MMTagMissingTerminator,
    #[error("Mismatch between sequence length and MN tag")]
    MNSeqLenMismatch,
    #[error("Mismatch between MM and ML tag lengths")]
    MMandMLLenMismatch,
    #[error("{0}")]
    General(String),
}