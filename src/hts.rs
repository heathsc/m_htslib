use libc::{c_char, c_int, c_uint, c_void};
use std::ffi::CStr;

pub mod hfile;
pub mod hts_error;
pub mod hts_format;
pub mod hts_idx;
pub mod hts_ocstr;
pub mod hts_opt;
pub mod hts_thread_pool;
pub mod htsfile;
pub mod traits;

pub use hfile::*;
pub use hts_format::*;
pub use hts_idx::*;
// pub use hts_ocstr::*;
pub use hts_opt::*;
pub use hts_thread_pool::*;
pub use htsfile::*;

use hts_error::HtsError;
use hts_ocstr::OCStr;

use crate::LIBHTS;

pub type HtsPos = i64;

#[repr(C)]
pub enum HtsLogLevel {
    Off,
    Error,
    Warning = 3,
    Info,
    Debug,
    Trace,
}

#[link(name = "hts")]
unsafe extern "C" {
    fn hts_version() -> *const c_char;
    fn hts_features() -> c_uint;
    fn hts_test_feature(feature: c_uint) -> *const c_char;
    fn hts_feature_string() -> *const c_char;
    fn hts_readlines(fn_: *const c_char, n: *mut c_int) -> *mut *mut c_char;
    fn hts_readlist(fn_: *const c_char, is_file: c_int, n: *mut c_int) -> *mut *mut c_char;
    fn hts_set_log_level(level: HtsLogLevel);
    fn hts_get_log_level() -> HtsLogLevel;
}

pub fn version() -> &'static CStr {
    let _guard = LIBHTS.read();
    unsafe { CStr::from_ptr(hts_version()) }
}

pub fn features() -> u32 {
    let _guard = LIBHTS.read();
    unsafe { hts_features() as u32 }
}

pub fn test_feature(feature: HtsFeature) -> &'static CStr {
    let _guard = LIBHTS.read();
    unsafe { CStr::from_ptr(hts_test_feature(1 << (feature as c_uint))) }
}

pub fn feature_string() -> &'static CStr {
    let _guard = LIBHTS.read();
    unsafe { CStr::from_ptr(hts_feature_string()) }
}

/// Read all lines from file `s` (or string `s` if `s` starts with ":" and a file of that name does not exist)
/// into a Boxed slice of OCStr.  When reading from a file, entries are separated by newlines, while when
/// reading from a string, entries are separated by commas.  Note that in the latter case, the first ':'
/// character is skipped.
pub fn read_lines(s: &CStr) -> Result<Box<[OCStr]>, HtsError> {
    let mut n: c_int = 0;
    let p = unsafe { hts_readlines(s.as_ptr(), &mut n) };
    try_make_boxed_slice(p, n)
}

///  Parse comma-separated list from `s` or read list from a file (one entry per line) named `s`.
/// The list is returned as a Boxed slice of OCStr
pub fn read_list(s: &CStr, is_file: bool) -> Result<Box<[OCStr]>, HtsError> {
    let mut n: c_int = 0;
    let p = unsafe { hts_readlist(s.as_ptr(), if is_file { 1 } else { 0 }, &mut n) };
    try_make_boxed_slice(p, n)
}

/// Sets log level for htslib, returning previous log level
pub fn set_log_level(level: HtsLogLevel) -> HtsLogLevel {
    let _guard = LIBHTS.write();
    unsafe { 
        let old = hts_get_log_level();
        hts_set_log_level(level);
        old
    }
}

pub fn get_log_level() -> HtsLogLevel {
    let _guard = LIBHTS.read();
    unsafe { hts_get_log_level() }
}

fn try_make_boxed_slice<'a>(p: *mut *mut c_char, n: c_int) -> Result<Box<[OCStr<'a>]>, HtsError> {
    if p.is_null() {
        Err(HtsError::OperationFailed)
    } else {
        assert!(n >= 0);
        Ok(
            unsafe {
                hts_ocstr::cstr_array_into_boxed_slice(p as *const *const c_char, n as usize)
            },
        )
    }
}

#[repr(C)]
#[derive(Debug)]
pub enum HtsFeature {
    // Whether configure was used or vanilla makefile
    Configure,
    // Whether --enable-plugins was used
    Plugins,

    // Transport specific
    Libcurl = 10,
    S3,
    GCS,

    // Compression options
    Libdeflate = 20,
    Lzma,
    Bzip2,
    Htscodecs, // htscodecs library version

    // Build params
    CC = 27,
    CFlags,
    CppFlags,
    LdFlags,
}

/// Whence argument for Seek calls
/// Set - relative to start of file
/// Cur - relative to current position
/// End - relative to end of file
#[derive(Copy, Clone, Debug)]
pub enum Whence {
    Set = libc::SEEK_SET as isize,
    Cur = libc::SEEK_CUR as isize,
    End = libc::SEEK_END as isize,
}

pub const HTS_IDX_DELIM: &str = "##idx##";

/// Not sure if I will use this, but if I do it won't be exposed to the public API
#[allow(dead_code)]
pub(crate) type HtsName2Id = unsafe extern "C" fn(hdr: *mut c_void, str: *const c_char) -> c_int;
