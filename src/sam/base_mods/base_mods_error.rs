use thiserror::Error;

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
}