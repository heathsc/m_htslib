use std::marker::PhantomData;

use libc::{size_t, c_char};

pub mod kstring_impl;
pub mod kstring_error;

#[repr(C)]
#[derive(Debug)]
struct RawString {
    l: size_t,
    m: size_t,
    s: *mut u8,
    marker: PhantomData<c_char>,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct KString {
    inner: RawString
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct MString {
    inner: RawString
}