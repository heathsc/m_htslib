use std::ffi::CStr;

/// Look up sequence (contig) internal ID in header dictionary (i.e., SAM/BAM/CRAM/VCF/BCF/TBX)
/// 
/// It is required that internal IDs are contiguous, starting from 0
pub trait SeqId {    
    /// Convert a sequence name (as a &`CStr`) to am internal id, returning None
    /// if the requested contig is not found.
    fn seq_id(&self, s: &CStr) -> Option<usize>;
}

/// Addtional conversions between contig names and ids
pub trait IdMap {  
    /// Get sequence name corresponding to an internal id
    fn seq_name(&self, i: usize) -> Option<&CStr>;
    
    /// Get sequence length corresponding to an internal id
    fn seq_len(&self, i: usize) -> Option<usize>;
    
    /// Get number of sequences in dictionary
    fn num_seqs(&self) -> usize;
}