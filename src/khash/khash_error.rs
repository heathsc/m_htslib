use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum KHashError {
    #[error("Could not allocate more memory")]
    OutOfMemory,
}
