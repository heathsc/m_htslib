use std::ffi::CStr;

use crate::{
    SamError,
    hts::HtsPos,
    sam::{BamRec, Cigar, CigarElem, cigar},
};

use libc::c_int;

impl BamRec {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn copy(&self, dst: &mut Self) {
        self.inner.copy(&mut dst.inner)
    }

    pub fn qname(&self) -> Option<&CStr> {
        if self.inner.data.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(self.inner.data) })
        }
    }

    #[inline]
    pub fn tid(&self) -> Option<usize> {
        check_tid(self.inner.core.tid)
    }

    #[inline]
    pub fn mtid(&self) -> Option<usize> {
        check_tid(self.inner.core.mtid)
    }

    pub fn cigar(&self) -> Option<&Cigar> {
        let len = self.inner.core.n_cigar as usize;
        if len > 0 {
            assert!(!self.inner.data.is_null());
            let slice = unsafe {
                let ptr = self.inner.data.offset(self.inner.core.l_qname as isize);
                assert_eq!(
                    ptr.align_offset(4),
                    0,
                    "Cigar storage not aligned - Bam record corrupt"
                );
                std::slice::from_raw_parts(ptr.cast::<CigarElem>(), len)
            };
            Some(unsafe { cigar::from_elems_unchecked(slice) })
        } else {
            None
        }
    }

    #[inline]
    pub fn endpos(&self) -> HtsPos {
        self.inner.end_pos()
    }

    #[inline]
    pub fn set_query_name(&mut self, qname: &CStr) -> Result<(), SamError> {
        self.inner.set_query_name(qname)
    }
}

#[inline]
fn check_tid(i: c_int) -> Option<usize> {
    if i >= 0 { Some(i as usize) } else { None }
}
