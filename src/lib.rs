#[macro_use]
extern crate log;

use std::sync::RwLock;

pub mod base;
pub mod bgzf;
pub mod cram;
pub mod error;
pub mod faidx;
pub(crate) mod gen_utils;
pub mod hts;
pub(crate) mod int_utils;
pub mod khash;
pub mod kstring;
pub mod le_bytes;
pub mod sam;

pub use error::*;
pub(crate) use gen_utils::*;
pub use le_bytes::LeBytes;

/// Controls access to global statics in libhts
struct LibHts();
static LIBHTS: RwLock<LibHts> = RwLock::new(LibHts());
