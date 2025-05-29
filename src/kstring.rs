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

/// KString - clone of htslib kstring
/// 
/// A clone of kstring from htslib. This used libc::malloc for allocation and is designed to
/// be interoperable with htslib, so that KString instanes made in Rust can be used, modified and freed
/// in C and visa versa. Unlike CString (which it resembles as it works with u8 rather than char), it is mutable 
/// so can be resized and data can be pushed to it. When a KString is changed (by pushing more characters or
/// by truncating etc.), it is guaranteed that there is one and only one zero contained in the string, and that
/// is at the end. In this way, we can convert it efficiently to/from a CStr.
#[repr(C)]
#[derive(Debug, Default)]
pub struct KString {
    inner: RawString
}

/// MString - simple u8 Vec using libc::malloc
/// 
/// Very similar to KString (and a lot of code is shared), except the restrictions about zero bytes are not present, so
/// null bytes can be present at any position (or be totally absent). This is therefore close to a Vec<u8>, except that
/// it uses malloc rather than the standard Rust allocator.
#[repr(C)]
#[derive(Debug, Default)]
pub struct MString {
    inner: RawString
}