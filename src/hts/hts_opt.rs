use libc::{c_char, c_int};
use std::{
    ffi::CStr,
    ops::{Deref, DerefMut},
    ptr,
};

use crate::{error::HtsError, hts::hts_format::HtsFmtOption};
use crate::hts::htsfile::HtsFile;

#[repr(C)]
union HtsOptVal {
    i: c_int,
    s: *mut c_char,
}

#[repr(C)]
pub struct HtsOptRaw {
    arg: *mut c_char,
    opt: HtsFmtOption,
    val: HtsOptVal,
    next: *mut HtsOptRaw,
}

#[link(name = "hts")]
extern "C" {
    fn hts_opt_free(opts: *mut HtsOptRaw);
    fn hts_opt_add(opts: *mut *mut HtsOptRaw, c_arg: *const c_char) -> c_int;
}

/// Note - inner *can* be null
#[repr(C)]
pub struct HtsOpt {
    inner: *mut HtsOptRaw,
}

impl Deref for HtsOpt {
    type Target = HtsOptRaw;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner }
    }
}

impl DerefMut for HtsOpt {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.inner }
    }
}
unsafe impl Send for HtsOpt {}
unsafe impl Sync for HtsOpt {}

impl Default for HtsOpt {
    fn default() -> Self {
        Self {
            inner: ptr::null_mut(),
        }
    }
}

impl Drop for HtsOpt {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            unsafe { hts_opt_free(self.inner) }
        }
    }
}

impl HtsOpt {
    /// Parses arg and adds it to HtsOpt
    #[inline]
    pub fn add(&mut self, arg: &CStr) -> Result<(), HtsError> {
        if unsafe { hts_opt_add(&mut self.inner, arg.as_ptr()) } == 0 {
            Ok(())
        } else {
            Err(HtsError::AddOptOperationFailed)
        }
    }
}
