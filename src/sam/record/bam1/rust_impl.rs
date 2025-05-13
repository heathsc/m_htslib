use super::*;

impl Drop for bam1_t {
    fn drop(&mut self) {
        // Note that we always own the struct (as indicated by mempolicy) so bam_destroy1() will free up the data field only
        unsafe { bam_destroy1(self) }
    }
}

impl Clone for bam1_t {
    fn clone(&self) -> Self {
        let mut new = Self::default();
        self.copy(&mut new);
        new
    }
} 
