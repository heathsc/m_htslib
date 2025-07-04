use std::ffi::CStr;

use crate::{
    hts::HtsPos, sam::{bam1::{bam1_t, BAM_FREVERSE}, BamRec, Cigar, CigarElem, QualIter, SeqIter, SeqQualIter}, SamError
};

use libc::c_int;

use super::{BAM_FMUNMAP, BAM_FUNMAP, bam1_core_t};

impl BamRec {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.inner.l_data = 0;
        self.inner.core = bam1_core_t::default();
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
            Some(unsafe { Cigar::from_elems_unchecked(slice) })
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

    #[inline]
    pub fn is_reversed(&self) -> bool {
        self.flag() & BAM_FREVERSE != 0
    }
    
    #[inline]
    pub fn is_mapped(&self) -> bool {
        self.flag() & BAM_FUNMAP == 0
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

    pub(super) fn make_data_slice(&self, off: usize, sz: usize) -> &[u8] {
        assert!(off + sz <= self.inner.l_data as usize, "Bam data corrupt");
        unsafe { super::make_data_slice(self.inner.data as *const u8, off, sz) }
    }

    pub fn seq_len(&self) -> usize {
        self.inner.core.l_qseq as usize
    }
    
    fn seq_slice(&self) -> &[u8] {
        let core = &self.inner.core;
        let off = ((core.n_cigar as usize) << 2) + core.l_qname as usize;
        self.make_data_slice(off, ((core.l_qseq + 1) >> 1) as usize)
    }

    pub fn qual_slice(&self) -> &[u8] {
        let core = &self.inner.core;
        let off = ((core.n_cigar as usize) << 2)
            + core.l_qname as usize
            + ((core.l_qseq + 1) >> 1) as usize;
        self.make_data_slice(off, core.l_qseq as usize)
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

    
    pub(crate) fn as_mut_ptr(&mut self) -> *mut bam1_t {
        &mut self.inner as *mut bam1_t
    }
}

unsafe impl Send for BamRec {}
unsafe impl Sync for BamRec {}

#[inline]
fn check_tid(i: c_int) -> Option<usize> {
    if i >= 0 { Some(i as usize) } else { None }
}
