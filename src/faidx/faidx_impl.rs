use std::{
    ffi::{CStr, CString},
    ops::{Deref, DerefMut},
    os::unix::ffi::OsStrExt,
    path::Path,
    ptr::{NonNull, null},
};

use libc::{c_char, c_int, c_void, free};

use crate::{
    FaidxError,
    bgzf::BgzfRaw,
    from_c,
    hts::{
        HtsPos,
        traits::{IdMap, SeqId},
    },
    khash::{KHashMap, KHashMapRaw},
};

use super::{Faidx, Sequence};

#[derive(Debug)]
#[repr(C)]
pub struct Faidx1Raw {
    id: c_int,
    line_len: u32,
    line_blen: u32,
    len: u64,
    seq_offset: u64,
    qual_offset: u64,
}

#[repr(C)]
pub struct FaidxRaw {
    bgzf: *mut BgzfRaw,
    n: c_int,
    m: c_int,
    name: *mut *mut c_char,
    hash: *mut KHashMapRaw<*const c_char, Faidx1Raw>,
    _unused: [u8; 0],
}

unsafe extern "C" {
    fn fai_load(fn_: *const c_char) -> *mut FaidxRaw;
    fn fai_load3(
        fn_: *const c_char,
        fnai: *const c_char,
        fngzi: *const c_char,
        flags: c_int,
    ) -> *mut FaidxRaw;
    fn faidx_nseq(fai: *const FaidxRaw) -> c_int;
    fn faidx_iseq(fai: *const FaidxRaw, n: c_int) -> *const c_char;
    fn faidx_seq_len64(fai: *const FaidxRaw, seq: *const c_char) -> HtsPos;
    fn faidx_fetch_seq64(
        fai: *const FaidxRaw,
        cname: *const c_char,
        x: HtsPos,
        y: HtsPos,
        len: *mut HtsPos,
    ) -> *mut c_char;
}

impl FaidxRaw {
    fn nseq(&self) -> usize {
        let l = unsafe { faidx_nseq(self) };
        l as usize
    }
    fn iseq(&self, i: usize) -> Option<&CStr> {
        if i > self.nseq() {
            panic!("Sequence ID {} out of range", i);
        }
        from_c(unsafe { faidx_iseq(self, i as libc::c_int) })
    }

    pub fn get_seq_len<S: AsRef<CStr>>(&self, cname: S) -> Option<usize> {
        let cname = cname.as_ref();
        let len = unsafe { faidx_seq_len64(self, cname.as_ref().as_ptr()) };
        if len < 0 { None } else { Some(len as usize) }
    }

    // Attempts to load reference sequence from file
    // x and y are 1 offset coordinates.  Setting x to 0 or 1 will load from the start of the contig.  Setting y to None
    // or a very large value will load until the end of the chromosome.
    // Returns errors if the chromosome is not found, the coordinates are invalid (i.e., y < x) or an IO error occurred
    pub fn fetch_seq<S: AsRef<CStr>>(
        &self,
        cname: S,
        x: usize,
        y: Option<usize>,
    ) -> Result<Sequence, FaidxError> {
        let cname = cname.as_ref();
        if let Some(seq_len) = self.get_seq_len(cname) {
            let y = y.map(|z| z.min(seq_len)).unwrap_or(seq_len);
            let x = x.saturating_sub(1);
            if y <= x {
                Err(FaidxError::IllegalInput)
            } else {
                let mut len: HtsPos = 0;
                let seq = unsafe {
                    faidx_fetch_seq64(
                        self,
                        cname.as_ptr(),
                        x as HtsPos,
                        (y - 1) as HtsPos,
                        &mut len,
                    )
                };
                if len == -2 {
                    Err(FaidxError::UnknownSequence)
                } else if len < 0 || seq.is_null() {
                    Err(FaidxError::ErrorLoadingSequence)
                } else {
                    Ok(Sequence {
                        inner: NonNull::new(seq as *mut u8).unwrap(),
                        start: x + 1,
                        len: len as usize,
                    })
                }
            }
        } else {
            Err(FaidxError::UnknownSequence)
        }
    }
}

unsafe impl Send for Faidx {}

impl Deref for Faidx {
    type Target = FaidxRaw;
    #[inline]
    fn deref(&self) -> &FaidxRaw {
        unsafe { self.inner.as_ref() }
    }
}

impl DerefMut for Faidx {
    #[inline]
    fn deref_mut(&mut self) -> &mut FaidxRaw {
        unsafe { self.inner.as_mut() }
    }
}

impl Faidx {
    pub fn load<S: AsRef<Path>>(name: S) -> Result<Faidx, FaidxError> {
        // If this fails then it is an error in the Rust std library!
        let cname = CString::new(name.as_ref().as_os_str().as_bytes()).unwrap();

        match NonNull::new(unsafe { fai_load3(cname.as_ptr(), null(), null(), 0) }) {
            None => Err(FaidxError::ErrorLoadingFaidx),
            Some(idx) => Ok(Faidx { inner: idx }),
        }
    }

    pub fn load_or_create<S: AsRef<Path>>(name: S) -> Result<Faidx, FaidxError> {
        let cname = CString::new(name.as_ref().as_os_str().as_bytes()).unwrap();

        match NonNull::new(unsafe { fai_load(cname.as_ptr()) }) {
            None => Err(FaidxError::ErrorLoadingFaidx),
            Some(idx) => Ok(Faidx { inner: idx }),
        }
    }
}

impl SeqId for Faidx {
    fn seq_id(&self, s: &CStr) -> Option<usize> {
        let hash = unsafe { KHashMap::from_raw_ptr(self.hash) };
        hash.get(&(s.to_bytes_with_nul().as_ptr() as *const c_char))
            .map(|f| f.id as usize)
    }
}
    
impl IdMap for Faidx {
    fn num_seqs(&self) -> usize {
        self.nseq()
    }
    
    fn seq_name(&self, i: usize) -> Option<&CStr> {
        self.iseq(i)
    }
    
    fn seq_len(&self, i: usize) -> Option<usize> {
        self.iseq(i).and_then(|s| self.get_seq_len(s))
    }
}

unsafe impl Send for Sequence {}
unsafe impl Sync for Sequence {}

impl Drop for Sequence {
    fn drop(&mut self) {
        unsafe { free(self.inner.as_ptr() as *mut c_void) }
    }
}

impl Sequence {
    // Get sequence between x and y inclusive (1 offset)
    pub fn get_seq(&self, x: usize, y: usize) -> Result<&[u8], FaidxError> {
        if x < 1 || x < self.start || x > y {
            Err(FaidxError::IllegalInput)
        } else {
            let a = x - self.start;
            let b = (y + 1 - self.start).min(self.len);
            let slice = self.seq();
            Ok(&slice[a..b])
        }
    }

    // Get entire loaded sequence as a slice
    pub fn seq(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.inner.as_ptr(), self.len) }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_faidx() {
        let h = Faidx::load("test/xx.fa").unwrap();
        let l = h.get_seq_len(c"yy");
        assert_eq!(l, Some(20));
        let l1 = h.seq_len(1);
        assert_eq!(l, l1);
        assert_eq!(h.num_seqs(), 5);
        assert_eq!(h.seq_name(1), Some(c"yy"));
        assert_eq!(h.seq_id(c"yy"), Some(1));
    }
}
