use std::ptr::NonNull;

pub mod faidx_error;
pub mod faidx_impl;

use faidx_impl::{FaidxRaw, SeqStore};

#[derive(Debug)]
pub struct Faidx {
    inner: NonNull<FaidxRaw>,
}

#[derive(Clone)]
pub struct Sequence {
    inner: SeqStore,
    start: usize,
}
