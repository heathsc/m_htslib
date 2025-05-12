use super::*;
use crate::sam::{cigar, Cigar, CigarElem};

impl Drop for bam1_t {
    fn drop(&mut self) {
        // Note that we always own the struct (as indicated by mempolicy) so bam_destroy1() will free up the data field only
        unsafe { bam_destroy1(self) }
    }
}

impl Clone for bam1_t {
    fn clone(&self) -> Self {
        let mut new = Self::default();
        if unsafe { bam_copy1(&mut new, self) }.is_null() {
            panic!("Out of memory copying Bam record")
        }
        new
    }
}

impl bam1_t {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cigar(&self) -> Option<&Cigar> {
        let len = self.core.n_cigar as usize;
        if len > 0 {
            assert!(!self.data.is_null());
            let slice = unsafe {
                let ptr = self.data.offset(self.core.l_qname as isize);
                assert_eq!(
                    ptr.align_offset(4),
                    0,
                    "Cigar storage not aligned - Bam record corrupt"
                );
                std::slice::from_raw_parts(ptr.cast::<CigarElem>(), len)
            };
            Some(unsafe {cigar::from_elems_unchecked(slice)} )
        } else {
            None
        }
    }
}
