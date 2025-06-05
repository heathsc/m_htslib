use std::ffi::CStr;

#[inline]
pub(crate) fn from_c<'a>(c: *const libc::c_char) -> Option<&'a CStr> {
    if c.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(c) })
    }
}

#[inline]
pub(crate) fn cstr_len(c: &CStr) -> usize {
    c.count_bytes()
}

/// Round up to next power of 2 unless this exceeds the maximum of usize, in which case use usize::MAX
/// This is a rust re-working of the kroundup32/64 macros from htslib
#[inline]
pub(crate) fn roundup(x: usize) -> usize {
    x.checked_next_power_of_two().unwrap_or(usize::MAX)
}
