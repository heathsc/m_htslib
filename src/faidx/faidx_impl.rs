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
        HtsPos, HtsTPoolRaw, HtsThreadPool,
        traits::{HdrType, HtsHdrType, IdMap, SeqId},
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
#[derive(Debug)]
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
    fn fai_build(fn_: *const c_char) -> c_int;
    fn faidx_nseq(fai: *const FaidxRaw) -> c_int;
    fn faidx_iseq(fai: *const FaidxRaw, n: c_int) -> *const c_char;
    fn faidx_seq_len64(fai: *const FaidxRaw, seq: *const c_char) -> HtsPos;
    /// Note - in htslib/faidx.h this has fai as a const pointer, but there is uncontrolled
    /// interior mutability w.r.t. the underlying file ptr, so we mark it as a mut pointer
    fn faidx_fetch_seq64(
        fai: *mut FaidxRaw,
        cname: *const c_char,
        x: HtsPos,
        y: HtsPos,
        len: *mut HtsPos,
    ) -> *mut c_char;
    fn fai_destroy(fai: *mut FaidxRaw);
    fn faidx_has_seq(fai: *const FaidxRaw, cname: *const c_char) -> c_int;
    unsafe fn fai_thread_pool(fai: *mut FaidxRaw, tpool: *const HtsTPoolRaw, qsize: c_int)
    -> c_int;
}

impl FaidxRaw {
    fn nseq(&self) -> usize {
        let l = unsafe { faidx_nseq(self) };
        l as usize
    }
    fn iseq(&self, i: usize) -> Option<&CStr> {
        if i > self.nseq() {
            panic!("Sequence ID {i} out of range");
        }
        from_c(unsafe { faidx_iseq(self, i as libc::c_int) })
    }

    pub fn has_seq<S: AsRef<CStr>>(&self, cname: S) -> bool {
        let cname = cname.as_ref();
        let ret = unsafe { faidx_has_seq(self as *const FaidxRaw, cname.as_ptr()) };
        ret == 1
    }

    pub fn get_seq_len<S: AsRef<CStr>>(&self, cname: S) -> Option<usize> {
        let cname = cname.as_ref();
        let len = unsafe { faidx_seq_len64(self as *const FaidxRaw, cname.as_ptr()) };
        if len < 0 { None } else { Some(len as usize) }
    }

    pub fn set_thread_pool(&mut self, thread_pool: &HtsThreadPool) {
        unsafe {
            fai_thread_pool(
                self as *mut FaidxRaw,
                thread_pool.deref(),
                thread_pool.size() as c_int,
            )
        };
    }

    // Attempts to load reference sequence from file
    // x and y are 1 offset coordinates.  Setting x to 0 or 1 will load from the start of the contig.  Setting y to None
    // or a very large value will load until the end of the chromosome.
    // Returns errors if the chromosome is not found, the coordinates are invalid (i.e., y < x) or an IO error occurred
    //
    // Note: even though the htslib call faidx_ftch_seq64() is marked as taking a const ptr to FaidxRaw, it has interior mutability
    // w.r.t. the underlying file pointer. We therefore mark it as requiring &mut self to prevent sharing across threads using Arc
    pub fn fetch_seq<S: AsRef<CStr>>(
        &mut self,
        cname: S,
        x: usize,
        y: Option<usize>,
    ) -> Result<Sequence, FaidxError> {
        let cname = cname.as_ref();
        if let Some(seq_len) = self.get_seq_len(cname) {
            let y = y.map(|z| z.min(seq_len)).unwrap_or(seq_len);
            let x = x.saturating_sub(1);
            if y <= x {
                Err(FaidxError::IllegalInput(x, y))
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
                    Ok(Sequence::from_ptr(seq as *const u8, len as usize, x))
                }
            }
        } else {
            Err(FaidxError::UnknownSequence)
        }
    }
}

unsafe impl Send for Faidx {}
unsafe impl Sync for Faidx {}

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

impl Drop for Faidx {
    fn drop(&mut self) {
        unsafe { fai_destroy(self.deref_mut()) }
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

    pub fn build<S: AsRef<Path>>(name: S) -> Result<(), FaidxError> {
        // If this fails then it is an error in the Rust std library!
        let cname = CString::new(name.as_ref().as_os_str().as_bytes()).unwrap();

        match unsafe { fai_build(cname.as_ptr()) } {
            0 => Ok(()),
            _ => Err(FaidxError::ErrorBuildingFaidx),
        }
    }
}

impl HdrType for Faidx {
    fn hdr_type(&self) -> HtsHdrType {
        HtsHdrType::Faidx
    }
}

impl SeqId for Faidx {
    fn seq_id(&self, s: &CStr) -> Option<usize> {
        let hash = unsafe { KHashMap::from_raw_ptr(self.hash) }.leak();
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

pub(super) enum SeqStore {
    CPtr(NonNull<u8>, usize),
    Slice(Box<[u8]>),
}

impl SeqStore {
    fn seq(&self) -> &[u8] {
        match self {
            Self::CPtr(p, len) => unsafe { std::slice::from_raw_parts(p.as_ptr(), *len) },
            Self::Slice(s) => s.as_ref(),
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::CPtr(_, len) => *len,
            Self::Slice(s) => s.len(),
        }
    }
}

unsafe impl Send for Sequence {}
unsafe impl Sync for Sequence {}

impl Drop for Sequence {
    fn drop(&mut self) {
        if let SeqStore::CPtr(p, _) = self.inner {
            unsafe { free(p.as_ptr() as *mut c_void) }
        }
    }
}

impl Sequence {
    pub fn from_boxed_slice(slice: Box<[u8]>, offset: usize) -> Self {
        Self {
            inner: SeqStore::Slice(slice),
            start: offset + 1,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn from_slice(slice: &[u8], offset: usize) -> Self {
        Self::from_boxed_slice(slice.to_owned().into_boxed_slice(), offset)
    }

    pub fn from_ptr(ptr: *const u8, len: usize, offset: usize) -> Self {
        Self {
            inner: SeqStore::CPtr(NonNull::new(ptr as *mut u8).unwrap(), len),
            start: offset + 1,
        }
    }

    // Get sequence between x and y inclusive (1 offset)
    pub fn get_seq(&self, x: usize, y: usize) -> Result<&[u8], FaidxError> {
        if x < 1 || x < self.start || x > y {
            Err(FaidxError::IllegalInput(x, y))
        } else {
            let a = x - self.start;
            let slice = self.seq();
            let len = self.len();
            Ok(if a >= len {
                &[]
            } else {
                let b = (y + 1 - self.start).min(len);
                &slice[a..b]
            })
        }
    }

    // Get entire loaded sequence as a slice
    #[inline]
    pub fn seq(&self) -> &[u8] {
        self.inner.seq()
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
        let mut h = Faidx::load("test/xx.fa").unwrap();
        let tp = HtsThreadPool::init(2).expect("Could not create threadpool");
        
        h.set_thread_pool(&tp);
        let l = h.get_seq_len(c"yy");
        assert_eq!(l, Some(20));
        let l1 = h.seq_len(1);
        assert_eq!(l, l1);
        assert_eq!(h.num_seqs(), 5);
        assert_eq!(h.seq_name(1), Some(c"yy"));
        assert_eq!(h.seq_id(c"zz"), Some(2));

        let s = h.fetch_seq(c"zz", 0, None).unwrap();
        assert_eq!(s.len(), 30);

        let s1 = s.get_seq(7, 14).unwrap();
        assert_eq!(s1, b"AAAATTTT");
    }
}
