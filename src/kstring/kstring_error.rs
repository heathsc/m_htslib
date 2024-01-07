use thiserror::Error;

#[derive(Error, Debug)]
pub enum KStringError {
    #[error("Could not allocate more memory")]
    OutOfMemory,
    #[error("Size request is too large")]
    SizeRequestTooLarge,
    #[error("Internal null character in supplied slice")]
    InternalNullInSlice,
}
