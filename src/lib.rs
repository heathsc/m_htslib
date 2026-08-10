#[macro_use]
extern crate log;

use std::sync::RwLock;

pub use m_htslib_rust::{base, kstring};

pub mod bgzf;
pub mod cram;
pub mod error;
pub mod faidx;
pub mod hts;
pub mod region;
pub mod sam;

pub use error::*;
pub use m_htslib_rust::{int_utils, gen_utils, sam::cigar, khash, le_bytes};
pub use le_bytes::LeBytes;

/// Controls access to global statics in libhts
struct LibHts();
static LIBHTS: RwLock<LibHts> = RwLock::new(LibHts());
