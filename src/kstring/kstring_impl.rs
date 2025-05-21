use std::{
    ffi::CStr,
    fmt,
    io::{self, Write},
    ptr,
    str::FromStr,
};

use crate::error::KStringError;
use libc::{c_char, c_void, size_t};

#[repr(C)]
#[derive(Debug)]
pub struct KString {
    l: size_t,
    m: size_t,
    s: *mut c_char,
}

impl PartialEq for KString {
    fn eq(&self, other: &Self) -> bool {
        self.l == other.l
            && (self.l == 0
                || !unsafe {
                    libc::memcmp(self.s as *const c_void, other.s as *const c_void, self.l) == 0
                })
    }
}

impl Eq for KString {}

impl fmt::Display for KString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_cstr().to_string_lossy())
    }
}

/// Because this has to interact with htslib which can alloc or free the storage
/// we need to use malloc/free from libc for all memory handling
impl Default for KString {
    fn default() -> Self {
        Self {
            l: 0,
            m: 0,
            s: ptr::null_mut(),
        }
    }
}

impl Drop for KString {
    fn drop(&mut self) {
        if !self.s.is_null() {
            unsafe { libc::free(self.s as *mut c_void) }
        }
    }
}

unsafe impl Send for KString {}
unsafe impl Sync for KString {}

impl Write for KString {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.putsn(buf).map_err(io::Error::other).map(|_| buf.len())
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.putsn(buf).map_err(io::Error::other)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl KString {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> size_t {
        self.l
    }

    pub fn is_empty(&self) -> bool {
        self.l == 0
    }

    pub fn clear(&mut self) {
        self.l = 0
    }

    pub fn resize(&mut self, size: size_t) -> Result<(), KStringError> {
        if self.m < size {
            let size = crate::roundup(size);
            let p = if self.s.is_null() {
                unsafe { libc::malloc(size) }
            } else {
                unsafe { libc::realloc(self.s as *mut c_void, size) }
            }
            .cast::<c_char>();

            if p.is_null() {
                return Err(KStringError::OutOfMemory);
            } else {
                self.s = p;
                self.m = size;
                unsafe { *p.add(self.l) = 0 }
            }
        }
        Ok(())
    }

    pub fn expand(&mut self, extra: usize) -> Result<(), KStringError> {
        if let Some(new_size) = self.l.checked_add(extra) {
            self.resize(new_size)
        } else {
            Err(KStringError::SizeRequestTooLarge)
        }
    }

    pub fn putsn(&mut self, p: &[u8]) -> Result<(), KStringError> {
        if !p.is_empty() {
            if p.contains(&0) {
                return Err(KStringError::InternalNullInSlice);
            }
            let l = p.len();
            self.expand(l + 2)?;
            unsafe {
                let ptr = self.s.add(self.l);
                libc::memcpy(ptr as *mut c_void, p.as_ptr() as *const c_void, l);
                self.l += l;
                *(ptr.add(l)) = 0;
            }
        }
        Ok(())
    }

    pub fn putc(&mut self, c: u8) -> Result<(), KStringError> {
        self.expand(2)?;
        unsafe {
            *self.s.add(self.l) = c as c_char;
            *self.s.add(self.l + 1) = 0;
        }
        self.l += 1;
        Ok(())
    }

    #[inline]
    pub fn as_cstr(&self) -> &CStr {
        unsafe { CStr::from_bytes_with_nul_unchecked(self._as_slice(true)) }
    }

    #[inline]
    fn _as_slice(&self, inc_zero: bool) -> &[u8] {
        if self.s.is_null() {
            if inc_zero { &[0] } else { &[] }
        } else {
            let p = self.s as *const u8;
            unsafe { std::slice::from_raw_parts(p, if inc_zero { self.l + 1 } else { self.l }) }
        }
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        self._as_slice(false)
    }

    #[inline]
    pub fn as_slice_with_null(&self) -> &[u8] {
        self._as_slice(true)
    }

    #[inline]
    pub fn to_str(&self) -> Result<&str, KStringError> {
        std::str::from_utf8(self.as_slice()).map_err(KStringError::Utf8Error)
    }
}

impl FromStr for KString {
    type Err = KStringError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut ks = Self::default();
        ks.putsn(s.as_bytes())?;
        Ok(ks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;

    #[test]
    fn construction() {
        let mut ks = KString::new();
        ks.putc(b'H').unwrap();
        ks.putc(b'e').unwrap();
        ks.putc(b'l').unwrap();
        assert_eq!(ks.len(), 3);
        let s = ks.as_slice();
        assert_eq!(s, b"Hel");
    }

    #[test]
    fn construction2() {
        let s = "Hello World".as_bytes();
        let mut ks = KString::new();
        ks.putsn(s).unwrap();
        assert_eq!(ks.len(), 11);

        ks.putsn(", and goodbye".as_bytes()).unwrap();
        assert_eq!(ks.len(), 24);
        assert_eq!(ks.as_cstr(), c"Hello World, and goodbye");
    }

    #[test]
    fn using_write() {
        let mut ks = KString::new();
        let x = 42;
        write!(ks, "Hello World. The number is {x}").unwrap();
        assert_eq!(ks.as_cstr(), c"Hello World. The number is 42");
    }
}
