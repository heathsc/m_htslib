use crate::{
    error::HtsError,
    hts::{cram_file_set_opt, HFile, HFileRaw, HtsFmtOption, Whence},
    sam::sam_hdr::SamHdrRaw,
    CramError,
};

use libc::{c_char, c_int, off_t};
use std::{
    ffi::CStr,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
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
pub struct CramRange {
    _unused: [u8; 0],
}

#[link(name = "hts")]
unsafe extern "C" {
    fn cram_open(path: *const c_char, mode: *const c_char) -> *mut CramFdRaw;
    fn cram_dopen(fp: *mut HFileRaw, fn_: *const c_char, mode: *const c_char) -> *mut CramFdRaw;
    fn cram_close(fp: *mut CramFdRaw) -> c_int;
    fn cram_fd_get_header(fd: *const CramFdRaw) -> *mut SamHdrRaw;
    fn cram_fd_get_version(fd: *const CramFdRaw) -> c_int;
    fn cram_major_vers(fd: *const CramFdRaw) -> c_int;
    fn cram_minor_vers(fd: *const CramFdRaw) -> c_int;
    fn cram_seek(fd: *mut CramFdRaw, off: off_t, whence: c_int) -> c_int;
    fn cram_flush(fd: *mut CramFdRaw) -> c_int;
    fn cram_eof(fd: *mut CramFdRaw) -> c_int;
    fn cram_set_header(fd: *mut CramFdRaw, hdr: *const SamHdrRaw) -> c_int;
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
    #[inline]
    pub fn major_version(&self) -> c_int {
        unsafe { cram_major_vers(self) }
    }
    #[inline]
    pub fn minor_version(&self) -> c_int {
        unsafe { cram_minor_vers(self) }
    }
    #[inline]
    pub fn seek(&mut self, off: off_t, whence: Whence) -> Result<(), CramError> {
        if unsafe { cram_seek(self, off, whence as c_int) } == 0 {
            Ok(())
        } else {
            Err(CramError::SeekFailed)
        }
    }
    pub fn flush(&mut self) -> Result<(), CramError> {
        if unsafe { cram_flush(self) } == 0 {
            Ok(())
        } else {
            Err(CramError::OperationFailed)
        }
    }
    pub fn eof(&mut self) -> Result<bool, CramError> {
        match unsafe { cram_eof(self) } {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(CramError::CannotCheckEOF),
        }
    }
    pub fn set_header(&mut self, hdr: &SamHdrRaw) -> Result<(), CramError> {
        if unsafe { cram_set_header(self, hdr) } == 0 {
            Ok(())
        } else {
            Err(CramError::OperationFailed)
        }
    }
    pub fn check_eof(&mut self) -> Result<(), CramError> {
        match unsafe { cram_check_EOF(self) } {
            0 => Err(CramError::MissingEOFMarker),
            1 => Ok(()),
            2 => Err(CramError::CannotCheckEOF),
            3 => Err(CramError::CramVersionHasNoEOF),
            _ => Err(CramError::IoError),
        }
    }
}

pub struct CramFd<'a> {
    inner: NonNull<CramFdRaw>,
    phantom: PhantomData<&'a mut CramFdRaw>,
}

impl Deref for CramFd<'_> {
    type Target = CramFdRaw;

    fn deref(&self) -> &Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { self.inner.as_ref() }
    }
}

impl DerefMut for CramFd<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { self.inner.as_mut() }
    }
}

unsafe impl Send for CramFd<'_> {}
unsafe impl Sync for CramFd<'_> {}

impl Drop for CramFd<'_> {
    fn drop(&mut self) {
        unsafe {
            cram_close(self.deref_mut());
        };
    }
}

impl CramFd<'_> {
    #[inline]
    pub fn open(name: &CStr, mode: &CStr) -> Result<Self, CramError> {
        Self::make_cram_file(unsafe { cram_open(name.as_ptr(), mode.as_ptr()) })
    }
    #[inline]
    pub fn dopen(fp: HFile, name: &CStr, mode: &CStr) -> Result<Self, CramError> {
        let ptr = fp.into_raw_ptr();
        Self::make_cram_file(unsafe { cram_dopen(ptr, name.as_ptr(), mode.as_ptr()) })
    }
    #[inline]
    fn make_cram_file(fp: *mut CramFdRaw) -> Result<Self, CramError> {
        match NonNull::new(fp) {
            Some(p) => {
                Ok(Self {
                    inner: p,
                    phantom: PhantomData,
                })
            }
            None => Err(CramError::OpenError)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_read() {
        // Try opening file
        let mut c =
            CramFd::open(c"test/test_input_1_a.cram", c"r").expect("Couldn't open CRAM file");
        // Test for closing EOF
        c.check_eof().expect("Missing EOF");
        // Get version
        let v = c.version();
        let i = c.major_version();
        let j = c.minor_version();
        assert_eq!((v, i, j), (768, 3, 0));
    }
}
