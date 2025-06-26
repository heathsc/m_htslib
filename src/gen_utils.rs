use std::{
    borrow::Cow,
    ffi::{CStr, CString},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

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

pub struct CStrWrap<'a> {
    inner: Cow<'a, CStr>,
}

impl CStrWrap<'_> {
    pub fn as_c_str(&self) -> &CStr {
        self.inner.as_ref()
    }
}

impl CStrWrap<'_> {
    pub fn as_ptr(&self) -> *const libc::c_char {
        self.inner.as_ref().as_ptr()
    }
}

impl<'a> From<&'a CStr> for CStrWrap<'a> {
    fn from(value: &'a CStr) -> Self {
        Self {
            inner: Cow::Borrowed(value),
        }
    }
}

impl<'a> From<&'a CString> for CStrWrap<'a> {
    fn from(value: &'a CString) -> Self {
        Self {
            inner: Cow::Borrowed(value.as_c_str()),
        }
    }
}

impl From<CString> for CStrWrap<'_> {
    fn from(value: CString) -> Self {
        Self {
            inner: Cow::Owned(value),
        }
    }
}

impl From<&str> for CStrWrap<'_> {
    fn from(value: &str) -> Self {
        Self {
            inner: Cow::Owned(CString::new(value).expect("Error converting to CString")),
        }
    }
}

impl From<&String> for CStrWrap<'_> {
    fn from(value: &String) -> Self {
        Self {
            inner: Cow::Owned(CString::new(value.as_str()).expect("Error converting to CString")),
        }
    }
}

impl From<String> for CStrWrap<'_> {
    fn from(value: String) -> Self {
        Self {
            inner: Cow::Owned(CString::new(value.as_str()).expect("Error converting to CString")),
        }
    }
}

impl From<&[u8]> for CStrWrap<'_> {
    fn from(value: &[u8]) -> Self {
        Self {
            inner: Cow::Owned(CString::new(value).expect("Error converting to CString")),
        }
    }
}

impl<const N: usize> From<&[u8; N]> for CStrWrap<'_> {
    fn from(value: &[u8; N]) -> Self {
        Self {
            inner: Cow::Owned(CString::new(value).expect("Error converting to CString")),
        }
    }
}

impl From<&Path> for CStrWrap<'_> {
    fn from(value: &Path) -> Self {
        let s = value.as_os_str().as_bytes();
        Self {
            inner: Cow::Owned(CString::new(s).expect("Error converting to CString")),
        }
    }
}

impl From<&PathBuf> for CStrWrap<'_> {
    fn from(value: &PathBuf) -> Self {
        let s = value.as_os_str().as_bytes();
        Self {
            inner: Cow::Owned(CString::new(s).expect("Error converting to CString")),
        }
    }
}

impl From<PathBuf> for CStrWrap<'_> {
    fn from(value: PathBuf) -> Self {
        let s = value.as_os_str().as_bytes();
        Self {
            inner: Cow::Owned(CString::new(s).expect("Error converting to CString")),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused)]

    use super::*;

    #[test]
    fn test_c_str_wrap() {
        let c: CStrWrap = "test".into();
        assert_eq!(c.as_c_str(), c"test");
        let c: CStrWrap = "test".as_bytes().into();
        assert_eq!(c.as_c_str(), c"test");
        let c: CStrWrap = PathBuf::from("dir/test").into();
        assert_eq!(c.as_c_str(), c"dir/test");
    }
}
