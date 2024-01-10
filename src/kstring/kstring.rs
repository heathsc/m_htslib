use std::{ffi::CStr, ptr};

use crate::error::KStringError;
use libc::{c_char, c_void, size_t};

#[repr(C)]
#[derive(Debug)]
pub struct KString {
    l: size_t,
    m: size_t,
    s: *mut c_char,
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
            let size = if size > (usize::MAX >> 2) {
                size
            } else {
                size + (size >> 1)
            };
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
            if p.iter().any(|c| *c == 0) {
                return Err(KStringError::InternalNullInSlice);
            }
            let l = p.len();
            self.expand(l + 2)?;
            unsafe {
                let ptr = self.s.add(self.l);
                libc::memcpy(ptr as *mut c_void, p.as_ptr() as *const c_void, l);
                self.l += l;
                *(ptr.offset(l as isize)) = 0;
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

    pub fn to_cstr(&self) -> Option<&CStr> {
        if self.s.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(self.s) })
        }
    }

    pub fn _as_slice(&self, inc_zero: bool) -> Option<&[u8]> {
        if self.s.is_null() {
            None
        } else {
            let p = self.s as *const u8;
            Some(unsafe {
                std::slice::from_raw_parts(p, if inc_zero { self.l + 1 } else { self.l })
            })
        }
    }

    pub fn _as_slice_mut(&mut self, inc_zero: bool) -> Option<&mut [u8]> {
        if self.s.is_null() {
            None
        } else {
            let p = self.s as *mut u8;
            Some(unsafe {
                std::slice::from_raw_parts_mut(p, if inc_zero { self.l + 1 } else { self.l })
            })
        }
    }

    pub fn as_slice(&self) -> Option<&[u8]> {
        self._as_slice(false)
    }

    pub fn as_slice_with_null(&self) -> Option<&[u8]> {
        self._as_slice(true)
    }

    pub fn as_slice_mut(&mut self) -> Option<&mut [u8]> {
        self._as_slice_mut(false)
    }

    pub fn as_slice_mut_with_null(&mut self) -> Option<&mut [u8]> {
        self._as_slice_mut(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction() {
        let mut ks = KString::new();
        ks.putc(b'H').unwrap();
        ks.putc(b'e').unwrap();
        ks.putc(b'l').unwrap();
        assert_eq!(ks.len(), 3);
        let s = ks.as_slice().unwrap();
        assert_eq!(s, &[b'H', b'e', b'l']);
    }

    #[test]
    fn construction2() {
        let s = "Hello World".as_bytes();
        let mut ks = KString::new();
        ks.putsn(s).unwrap();
        assert_eq!(ks.len(), 11);
    }
}
