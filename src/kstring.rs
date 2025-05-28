use std::marker::PhantomData;

use libc::{size_t, c_char};

pub mod kstring_impl;
pub mod kstring_error;

#[repr(C)]
#[derive(Debug)]
pub struct KString {
    l: size_t,
    m: size_t,
    s: *mut c_char,
    marker: PhantomData<c_char>,
}
