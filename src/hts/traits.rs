use std::{ffi::CStr, iter::FusedIterator};

/// Look up sequence (contig) internal ID in header dictionary (i.e., SAM/BAM/CRAM/VCF/BCF/TBX)
///
/// It is required that internal IDs are contiguous, starting from 0
pub trait SeqId {
    /// Convert a sequence name (as a &`CStr`) to am internal id, returning None
    /// if the requested contig is not found.
    fn seq_id(&self, s: &CStr) -> Option<usize>;
}

/// Addtional conversions between contig names and ids
pub trait IdMap: Sized {
    /// Get sequence name corresponding to an internal id
    fn seq_name(&self, i: usize) -> Option<&CStr>;

    /// Get sequence length corresponding to an internal id
    fn seq_len(&self, i: usize) -> Option<usize>;

    /// Get number of sequences in dictionary
    fn num_seqs(&self) -> usize;

    fn seq_iter(&self) -> impl Iterator<Item = &CStr> {
        SeqIter{hdr: self, ix: 0, end: self.num_seqs()}
    }
}

pub struct SeqIter<'a, T> {
    hdr: &'a T,
    ix: usize,
    end: usize,
}

impl <'a, T> Iterator for SeqIter<'a, T> 
where T: IdMap
{
    type Item = &'a CStr;
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.ix < self.end {
            let s = self.hdr.seq_name(self.ix);
            self.ix += 1;
            s
            
        } else {
            None
        }
    }
    
    fn size_hint(&self) -> (usize, Option<usize>) {
        let sz = self.end.saturating_sub(self.ix);
        (sz, Some(sz))
    }
    
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.ix = self.end.min(self.ix + n);
        self.next()
    }
}

impl <T: IdMap> ExactSizeIterator for SeqIter<'_, T> {}
impl <T: IdMap> FusedIterator for SeqIter<'_, T> {}
