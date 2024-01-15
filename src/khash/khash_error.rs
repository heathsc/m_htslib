use thiserror::Error;

#[derive(Error, Debug)]
pub enum KHashError {
    #[error("Could not allocate more memory")]
    OutOfMemory,
}
