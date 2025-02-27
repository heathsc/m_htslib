use libc::{c_char, c_void};
use std::{ffi::CStr, marker::PhantomData};

/// Owned CStr
/// The difference with CString is that the memory was allocated with libc::free(), and so needs
/// to be deallocated using libc::free().  An OCStr can only be made using `OCStr::from_ptr`, using
/// a valid pointer to C string allocated from C code.  Note that this a read only view of the C string.
///
/// We implement Deref so that it will inherit the methods from CStr
pub struct OCStr<'a> {
    inner: *const c_char,
    phantom: PhantomData<&'a c_char>,
}

impl Drop for OCStr<'_> {
    fn drop(&mut self) {
        unsafe { libc::free(self.inner as *mut c_void) }
    }
}

impl OCStr<'_> {
    /// Wrap a raw ptr to a C string in a OCStr.
    ///
    /// # Safety
    ///
    /// The pointer `p` *must* have been allocated using libc::malloc, must point to a valid,
    /// null terminated, C string, of size less then `isize::MAX`, and it should be the only
    /// reference to this block of memory
    #[inline]
    pub unsafe fn from_ptr(p: *const c_char) -> Self {
        assert!(!p.is_null());
        Self {
            inner: p,
            phantom: PhantomData,
        }
    }
    #[inline]
    pub fn as_ptr(&self) -> *const c_char {
        self.inner
    }
    #[inline]
    pub fn to_cstr(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.inner) }
    }
}

pub(crate) unsafe fn cstr_array_into_boxed_slice<'a>(
    p: *const *const c_char,
    n: usize,
) -> Box<[OCStr<'a>]> {
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let ptr = unsafe { *p.add(i) };
        v.push(unsafe { OCStr::from_ptr(ptr) })
    }
    v.into_boxed_slice()
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use super::*;
    
    fn make_c_allocated_string(s: &str) -> *mut c_char {
        let cs = CString::new(s).unwrap();
        let ptr = unsafe { libc::strdup(cs.as_ptr() as *mut c_char) };
        assert!(!ptr.is_null());
        ptr
    }

    #[test]
    fn construction() {
        let ptr = make_c_allocated_string("Hello World");
        let s = unsafe { OCStr::from_ptr(ptr) };
        let s1 = s.to_cstr();
        assert_eq!(s1, c"Hello World");
    }

    #[test]
    fn read_list_from_str() {
        let v = crate::hts::read_list(c"Test,item2,item3,item4,item5", false).unwrap();
        assert_eq!(v.len(), 5);
        assert_eq!(v[4].to_cstr(), c"item5");
    }

    #[test]
    fn read_list_from_file() {
        let v = crate::hts::read_list(c"test/list.txt", true).unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(v[2].to_cstr(), c"Carrot");
    }

    #[test]
    fn read_lines_from_file() {
        let v = crate::hts::read_lines(c"test/list.txt").unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(v[1].to_cstr(), c"Bee");
    }

    #[test]
    fn read_lines_from_str() {
        let s = c":Test,item2, item3 ,item4,item5";
        let v = crate::hts::read_lines(s).unwrap();
        assert_eq!(v.len(), 5);
        assert_eq!(v[2].to_cstr(), c" item3 ");
    }
}
