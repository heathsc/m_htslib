use std::{marker::PhantomData, ptr::NonNull};

pub mod faidx_error;
pub mod faidx_impl;

use faidx_impl::FaidxRaw;

#[derive(Debug)]
pub struct Faidx<'a> {
    inner: NonNull<FaidxRaw>,
    phantom: PhantomData<&'a mut FaidxRaw>,
}

pub struct Sequence {
    inner: NonNull<u8>,
    start: usize,
    len: usize,
}
