use std::ffi::CStr;

pub mod bgzf;
pub mod cram;
pub mod error;
pub mod hts;
pub mod khash;
pub mod kstring;
pub mod sam;

pub use error::*;

#[inline]
fn from_c<'a>(c: *const libc::c_char) -> Option<&'a CStr> {
    if c.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(c) })
    }
}

#[inline]
fn cstr_len(c: &CStr) -> usize {
    c.count_bytes()
}

/// Round up to next power of 2 unless this exceeds the maximum of usize, in which case use usize::MAX
/// This is a rust re-working of the kroundup32/64 macros from htslib
#[inline]
fn roundup(x: usize) -> usize {
    x.checked_next_power_of_two().unwrap_or(usize::MAX)
}

/*
#[inline]
fn from_cstr(c: *const libc::c_char) -> Result<&str, HtsError> {
    if c.is_null() {
        Err(HtsError::CStringNull)
    } else {
        unsafe { CStr::from_ptr(c) }
            .to_str()
            .map_err(|_| HtsError::CStringNotUTF8)
    }
}

 */
