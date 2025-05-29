use std::{ffi::FromBytesWithNulError, num::{ParseFloatError, ParseIntError}, str::Utf8Error, io};

use thiserror::Error;

use crate::ParseINumError;

#[derive(Error, Debug)]
pub enum AuxError {
    #[error("Internal Error")]
    InternalError,
    #[error("Aux tag too short (incomplete)")]
    ShortTag,
    #[error("Bad characters in tag ID ({0}, {1})")]
    BadCharsInTagId(u8, u8),
    #[error("Duplicate tag ID ({0}{1})")]
    DuplicateTagId(char, char),
    #[error("Bad aux tag format")]
    BadFormat,
    #[error("Bad BAM aux tag format ({0})")]
    BadBamTagFormat(u8),
    #[error("BAM aux tag corrupt")]
    CorruptBamTag,
    #[error("Bad A (single character) aux tag format")]
    BadAFormat,
    #[error("Illegal zero length aux tag")]
    ZeroLengthTag,
    #[error("Utf8 Error: {0}")]
    Utf8Error(#[from] Utf8Error),
    #[error("CStr Error: {0}")]
    CStrError(#[from] FromBytesWithNulError),
    #[error("Parse Int Error: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error("Parse Float Error: {0}")]
    ParseFloatError(#[from] ParseFloatError),
    #[error("Aux tag integer out of range")]
    IntegerOutOfRange,
    #[error("Hex digits does not have an even number of digits")]
    OddHexDigits,
    #[error("Illegal characters in Z or A aux tag")]
    IllegalCharacters,
    #[error("Non hex digits in H aux tag")]
    IllegalHexCharacters,
    #[error("Unknown aux tag type '{0}'")]
    UnknownType(char),
    #[error("Unknown aux array type '{0}'")]
    UnknownArrayType(char),
    #[error("Integer overflow")]
    IntegerOverflow((i64, i64)),
    #[error("Integer size too small")]
    IntegerTooSmall(u8),
    #[error("Parse Float Error")]
    FloatError,
    #[error("Parse number error: {0}")]
    INumError(#[from] ParseINumError),
    #[error("Parse number error: {0}")]
    IoError(#[from] io::Error),
}
