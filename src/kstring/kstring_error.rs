use std::str::Utf8Error;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum KStringError {
    #[error("Could not allocate more memory")]
    OutOfMemory,
    #[error("Size request is too large")]
    SizeRequestTooLarge,
    #[error("Internal null character in supplied slice")]
    InternalNullInSlice,
    #[error("Cannor push null character to KString")]
    InternalNull,
    #[error("Utf8 Error: {0}")]
    Utf8Error(#[from] Utf8Error),
}
