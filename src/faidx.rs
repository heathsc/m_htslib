use std::ptr::NonNull;

pub mod faidx_error;
pub mod faidx_impl;

use faidx_impl::FaidxRaw;

#[derive(Debug)]
pub struct Faidx {
    inner: NonNull<FaidxRaw>,
}

pub struct Sequence {
    inner: NonNull<u8>,
    start: usize,
    len: usize,
}
