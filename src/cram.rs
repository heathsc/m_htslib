pub mod cram_error;

use crate::{
    error::HtsError,
    hts::{cram_file_set_opt, HFileRaw, HtsFmtOption, Whence},
    sam::sam_hdr::SamHdrRaw,
};

use libc::{c_char, c_int, off_t};
use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

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
    fn cram_fd_get_header(fd: *const CramFdRaw) -> *mut SamHdrRaw;
    fn cram_fd_get_version(fd: *const CramFdRaw) -> c_int;
    fn cram_fd_major_version(fd: *const CramFdRaw) -> c_int;
    fn cram_fd_minor_version(fd: *const CramFdRaw) -> c_int;
    fn cram_fd_get_fp(fd: *const CramFdRaw) -> *mut HFileRaw;
    fn cram_fd_set_fp(fd: *mut CramFdRaw, hdr: *mut HFileRaw);
    fn cram_seek(fd: *mut CramFdRaw, off: off_t, whence: Whence) -> c_int;
    fn cram_flush(fd: *mut CramFdRaw) -> c_int;
    fn cram_eof(fd: *mut CramFdRaw) -> c_int;
    fn cram_set_header(fd: *mut CramFdRaw, hdr: *mut SamHdrRaw) -> c_int;
    fn cram_check_EOF(fd: *mut CramFdRaw) -> c_int;
}

impl CramFdRaw {
    #[inline]
    pub fn set_opt(&mut self, opt: &mut HtsFmtOption) -> Result<(), HtsError> {
        cram_file_set_opt(self, opt)
    }

    #[inline]
    pub fn get_header(&self) -> Option<&SamHdrRaw> {
        let p = unsafe { cram_fd_get_header(self) };
        if p.is_null() {
            None
        } else {
            Some(unsafe { &*p })
        }
    }

    #[inline]
    pub fn version(&self) -> c_int {
        unsafe { cram_fd_get_version(self) }
    }
}

pub struct CramFd<'a> {
    inner: *mut CramFdRaw,
    phantom: PhantomData<&'a CramFdRaw>,
}

impl<'a> Deref for CramFd<'a> {
    type Target = CramFdRaw;

    fn deref(&self) -> &Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { &*self.inner }
    }
}

impl<'a> DerefMut for CramFd<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { &mut *self.inner }
    }
}

unsafe impl<'a> Send for CramFd<'a> {}
unsafe impl<'a> Sync for CramFd<'a> {}

impl<'a> Drop for CramFd<'a> {
    fn drop(&mut self) {
        unsafe {
            cram_close(self.inner);
        };
    }
}
