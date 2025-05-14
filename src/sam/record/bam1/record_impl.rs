use std::ffi::CStr;

use crate::{
    SamError,
    hts::HtsPos,
    sam::{BamRec, Cigar, CigarElem, SeqIter, QualIter, SeqQualIter, cigar},
};

use libc::c_int;

use super::{BAM_FMUNMAP, BAM_FUNMAP};

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

    #[inline]
    pub fn mapq(&self) -> u8 {
        self.inner.core.qual
    }

    #[inline]
    pub fn flag(&self) -> u16 {
        self.inner.core.flag
    }

    pub fn pos(&self) -> Option<HtsPos> {
        let x = self.inner.core.pos;
        if x >= 0 && (self.inner.core.flag & BAM_FUNMAP) == 0 {
            Some(x)
        } else {
            None
        }
    }

    pub fn mpos(&self) -> Option<HtsPos> {
        let x = self.inner.core.mpos;
        if x >= 0 && (self.inner.core.flag & BAM_FMUNMAP) == 0 {
            Some(x)
        } else {
            None
        }
    }

    pub fn template_len(&self) -> HtsPos {
        self.inner.core.isze
    }

    fn seq_slice(&self) -> &[u8] {
        let b = &self.inner;
        let core = &b.core;
        if b.data.is_null() || core.l_qseq == 0 {
            &[]
        } else {
            let core = &b.core;
            let off = ((core.n_cigar as usize) << 2) + core.l_qname as usize;
            unsafe {
                std::slice::from_raw_parts(
                    b.data.add(off) as *const u8,
                    ((core.l_qseq + 1) >> 1) as usize,
                )
            }
        }
    }

    pub fn qual_slice(&self) -> &[u8] {
        let b = &self.inner;
        let core = &b.core;
        if b.data.is_null() || core.l_qseq == 0 {
            &[]
        } else {
            let core = &b.core;
            let off = ((core.n_cigar as usize) << 2)
                + core.l_qname as usize
                + ((core.l_qseq + 1) >> 1) as usize;
            unsafe {
                std::slice::from_raw_parts(
                    b.data.add(off) as *const u8,
                    core.l_qseq as usize,
                )
            }
        }
    }

    #[inline]
    pub fn seq(&self) -> SeqIter {
        SeqIter::new(self.seq_slice(), self.inner.core.l_qseq as usize)
    }
    
    #[inline]
    pub fn qual(&self) -> QualIter {
        QualIter::new(self.qual_slice())
    }
    
    #[inline]
    pub fn seq_qual(&self) -> SeqQualIter {
        SeqQualIter::new(self.seq_slice(), self.qual_slice())
    }
}

#[inline]
fn check_tid(i: c_int) -> Option<usize> {
    if i >= 0 { Some(i as usize) } else { None }
}
