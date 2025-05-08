use super::*;

use libc::{c_int, c_void, realloc};

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

    /// In common with standard rust memory allocation, we panic if memory is not available
    /// or if allocation requested is too large
    pub fn realloc_data(&mut self, size: usize) {
        // Can only use this with htslib managed data
        assert_eq!(self.mempolicy & BAM_USER_OWNS_DATA, 0);
        let s = crate::roundup(size);
        assert!(
            s <= c_int::MAX as usize,
            "Requested allocation size is too large for Bam Record"
        );
        let new_data = unsafe { realloc(self.data as *mut c_void, s) };
        assert!(!new_data.is_null(), "Out of memory");

        self.data = new_data as *mut c_char;
        self.m_data = s as u32;
        self.l_data = self.l_data.min(s as c_int);
    }

    pub fn reserve(&mut self, additional: usize) {
        let sz = (self.l_data as usize)
            .checked_add(additional)
            .expect("Allocation size too high");
        if sz > self.m_data as usize {
            self.realloc_data(sz)
        }
    }
}
