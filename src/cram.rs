pub mod cram_error;

use crate::{
    error::HtsError,
    hts::{cram_file_set_opt, HFileRaw, HtsFmtOption},
};

use libc::{c_char, c_int};
use std::ops::{Deref, DerefMut};

#[repr(C)]
pub struct CramFdRaw {
    _unused: [u8; 0],
}

#[repr(C)]
pub struct Refs {
    _unused: [u8; 0],
}

#[repr(C)]
pub(crate) struct CramRange {
    _unused: [u8; 0],
}

#[link(name = "hts")]
extern "C" {
    fn cram_open(path: *const c_char, mode: *const c_char) -> *mut CramFdRaw;
    fn cram_dopen(fp: *mut HFileRaw, fn_: *const c_char, mode: *const c_char) -> *mut CramFdRaw;
    fn cram_close(fp: *mut CramFdRaw) -> c_int;
}

impl CramFdRaw {
    #[inline]
    pub fn set_opt(&mut self, opt: &mut HtsFmtOption) -> Result<(), HtsError> {
        cram_file_set_opt(self, opt)
    }
}

pub struct CramFd {
    inner: *mut CramFdRaw,
}

impl Deref for CramFd {
    type Target = CramFdRaw;

    fn deref(&self) -> &Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { &*self.inner }
    }
}

impl DerefMut for CramFd {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { &mut *self.inner }
    }
}

unsafe impl Send for CramFd {}
unsafe impl Sync for CramFd {}

impl Drop for CramFd {
    fn drop(&mut self) {
        unsafe {
            cram_close(self.inner);
        };
    }
}
