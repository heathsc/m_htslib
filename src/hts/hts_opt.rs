use libc::{c_char, c_int, c_void};
use std::{
    ffi::CStr,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr,
};

use crate::{error::HtsError, hts::hts_format::HtsFmtOptionRaw};

#[repr(C)]
union OptVal {
    i: c_int,
    ptr: *mut c_void,
}

#[repr(C)]
pub struct HtsOptRaw {
    arg: *mut c_char,
    opt: HtsFmtOptionRaw,
    val: OptVal,
    next: *mut HtsOptRaw,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum HtsProfileOption {
    Fast,
    Normal,
    Small,
    Archive,
}

#[link(name = "hts")]
unsafe extern "C" {
    fn hts_opt_free(opts: *mut HtsOptRaw);
    fn hts_opt_add(opts: *mut *mut HtsOptRaw, c_arg: *const c_char) -> c_int;
}

/// Note - inner *can* be null
#[repr(C)]
pub struct HtsOpt<'a> {
    inner: *mut HtsOptRaw,
    phantom: PhantomData<&'a HtsOptRaw>,
}

impl Deref for HtsOpt<'_> {
    type Target = HtsOptRaw;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner }
    }
}

impl DerefMut for HtsOpt<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.inner }
    }
}
unsafe impl Send for HtsOpt<'_> {}
unsafe impl Sync for HtsOpt<'_> {}

impl Default for HtsOpt<'_> {
    fn default() -> Self {
        Self {
            inner: ptr::null_mut(),
            phantom: PhantomData,
        }
    }
}

impl Drop for HtsOpt<'_> {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            unsafe { hts_opt_free(self.inner) }
        }
    }
}

impl HtsOpt<'_> {
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
