use std::{
    ffi::CStr,
    fmt,
    io::{self, Write},
    marker::PhantomData,
    ptr,
    str::FromStr,
};

use super::{KString, MString, RawString};

use crate::error::KStringError;
use libc::{c_void, size_t};

impl PartialEq for RawString {
    fn eq(&self, other: &Self) -> bool {
        self.l == other.l
            && (self.l == 0
                || !unsafe {
                    libc::memcmp(self.s as *const c_void, other.s as *const c_void, self.l) == 0
                })
    }
}

impl PartialEq for KString {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl PartialEq for MString {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for RawString {}
impl Eq for MString {}
impl Eq for KString {}

impl fmt::Display for KString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_cstr().to_string_lossy())
    }
}

/// Because this has to interact with htslib which can alloc or free the storage
/// we need to use malloc/free from libc for all memory handling
impl Default for RawString {
    fn default() -> Self {
        Self {
            l: 0,
            m: 0,
            s: ptr::null_mut(),
            marker: PhantomData,
        }
    }
}

impl Drop for RawString {
    fn drop(&mut self) {
        if !self.s.is_null() {
            unsafe { libc::free(self.s as *mut c_void) }
        }
    }
}

unsafe impl Send for RawString {}
unsafe impl Sync for RawString {}
unsafe impl Send for MString {}
unsafe impl Sync for MString {}
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

impl Write for MString {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.putsn(buf);
        Ok(buf.len())
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.putsn(buf);
        Ok(())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl RawString {
    #[inline]
    fn len(&self) -> size_t {
        self.l
    }

    #[inline]
    fn capacity(&self) -> size_t {
        self.m
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.l == 0
    }

    #[inline]
    fn clear(&mut self) {
        self.l = 0
    }

    #[inline]
    fn truncate(&mut self, l: usize) {
        let l = l as size_t;
        if l < self.l {
            self.l = l
        }
    }

    unsafe fn set_len(&mut self, l: usize) {
        assert!(l <= self.m);
        self.l = l
    }

    fn resize(&mut self, size: size_t) {
        if self.m < size {
            let size = crate::roundup(size);
            let p = if self.s.is_null() {
                unsafe { libc::malloc(size) }
            } else {
                unsafe { libc::realloc(self.s as *mut c_void, size) }
            }
            .cast::<u8>();

            assert!(!p.is_null(), "KString: Out of memory");

            self.s = p;
            self.m = size;
            unsafe { *p.add(self.l) = 0 }
        }
    }

    fn extend(&mut self, extra: usize) {
        if let Some(new_size) = self.l.checked_add(extra) {
            self.resize(new_size)
        } else {
            panic!("String resize too large")
        }
    }

    fn putsn(&mut self, p: &[u8]) {
        if !p.is_empty() {
            let l = p.len();
            self.extend(l);
            unsafe {
                let ptr = self.s.add(self.l);
                libc::memcpy(ptr as *mut c_void, p.as_ptr() as *const c_void, l);
                self.l += l;
            }
        }
    }

    fn putc(&mut self, c: u8) {
        self.extend(1);
        unsafe {
            *self.s.add(self.l) = c;
        }
        self.l += 1;
    }

    #[inline]
    fn as_slice(&self) -> &[u8] {
        if self.s.is_null() {
            &[]
        } else {
            let p = self.s as *const u8;
            unsafe { std::slice::from_raw_parts(p, self.l) }
        }
    }

    #[inline]
    fn to_str(&self) -> Result<&str, KStringError> {
        std::str::from_utf8(self.as_slice()).map_err(KStringError::Utf8Error)
    }

    #[inline]
    fn as_ptr(&self) -> *const u8 {
        self.s as *const u8
    }

    #[inline]
    fn as_ptr_mut(&mut self) -> *mut u8 {
        self.s
    }
}

impl KString {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn len(&self) -> size_t {
        self.inner.len()
    }

    #[inline]
    pub fn capacity(&self) -> size_t {
        self.inner.capacity()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear()
    }

    #[inline]
    pub fn truncate(&mut self, l: usize) {
        let l = l as size_t;
        let rs = &mut self.inner;
        if l < rs.l {
            rs.l = l;
            unsafe {
                *rs.s.add(rs.l + 1) = 0;
            }
        }
    }

    #[inline]
    pub fn resize(&mut self, size: size_t) {
        self.inner.resize(size)
    }

    #[inline]
    pub fn extend(&mut self, extra: usize) {
        self.inner.extend(extra)
    }

    pub fn putsn(&mut self, p: &[u8]) -> Result<(), KStringError> {
        let rs = &mut self.inner;
        if !p.is_empty() {
            if p.contains(&0) {
                return Err(KStringError::InternalNullInSlice);
            }

            let l = p.len();
            rs.extend(l + 1);
            unsafe {
                let ptr = rs.s.add(rs.l);
                libc::memcpy(ptr as *mut c_void, p.as_ptr() as *const c_void, l);
                rs.l += l;
                *(ptr.add(l)) = 0;
            }
        }
        Ok(())
    }

    pub fn putc(&mut self, c: u8) -> Result<(), KStringError> {
        if c == 0 {
            Err(KStringError::InternalNull)
        } else {
            let rs = &mut self.inner;
            rs.extend(2);
            unsafe {
                *rs.s.add(rs.l) = c;
                *rs.s.add(rs.l + 1) = 0;
            }
            rs.l += 1;
            Ok(())
        }
    }

    #[inline]
    pub fn as_cstr(&self) -> &CStr {
        unsafe { CStr::from_bytes_with_nul_unchecked(self.as_slice_with_null()) }
    }

    #[inline]
    fn as_slice_with_null(&self) -> &[u8] {
        if self.inner.s.is_null() {
            &[0]
        } else {
            let p = self.inner.s as *const u8;
            unsafe { std::slice::from_raw_parts(p, self.inner.l + 1) }
        }
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        self.inner.as_slice()
    }

    #[inline]
    pub fn to_str(&self) -> Result<&str, KStringError> {
        self.inner.to_str()
    }

    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.inner.as_ptr()
    }

    #[inline]
    pub fn as_ptr_mut(&mut self) -> *mut u8 {
        self.inner.as_ptr_mut()
    }
}

impl MString {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn len(&self) -> size_t {
        self.inner.len()
    }

    #[inline]
    pub fn capacity(&self) -> size_t {
        self.inner.capacity()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear()
    }

    #[inline]
    pub fn truncate(&mut self, l: usize) {
        self.inner.truncate(l)
    }

    /// Forces the length of the mstring to `new_len`.
    ///
    /// This is a low-level operation that maintains none of the normal
    /// invariants of the type. Normally changing the length of a mstring
    /// is done using one of the safe operations instead, such as
    /// [`truncate`], [`resize`], [`extend`], or [`clear`].
    ///
    /// [`truncate`]: MString::truncate
    /// [`resize`]: MString::resize
    /// [`extend`]: MString::extend
    /// [`clear`]: MString::clear
    ///
    /// # Safety
    ///
    /// - `new_len` must be less than or equal to [`capacity()`].
    /// - The elements at `old_len..new_len` must be initialized.
    ///
    /// [`capacity()`]: MString::capacity
    #[inline]
    pub unsafe fn set_len(&mut self, l: usize) {
        unsafe { self.inner.set_len(l) }
    }

    #[inline]
    pub fn resize(&mut self, size: size_t) {
        self.inner.resize(size)
    }

    #[inline]
    pub fn extend(&mut self, extra: usize) {
        self.inner.extend(extra)
    }

    pub fn putsn(&mut self, p: &[u8]) {
        self.inner.putsn(p)
    }

    pub fn putc(&mut self, c: u8) {
        self.inner.putc(c)
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        self.inner.as_slice()
    }

    #[inline]
    pub fn to_str(&self) -> Result<&str, KStringError> {
        self.inner.to_str()
    }

    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.inner.as_ptr()
    }

    #[inline]
    pub fn as_ptr_mut(&mut self) -> *mut u8 {
        self.inner.as_ptr_mut()
    }
}

impl FromStr for RawString {
    type Err = KStringError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut ks = Self::default();
        ks.putsn(s.as_bytes());
        Ok(ks)
    }
}

impl FromStr for MString {
    type Err = KStringError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        RawString::from_str(s).map(|inner| Self { inner })
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
        let _ = ks.putsn(s);
        assert_eq!(ks.len(), 11);

        let _ = ks.putsn(", and goodbye".as_bytes());
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
