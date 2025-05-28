use std::cmp::Ordering;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BDSection {
    QName = 0,
    Cigar,
    Seq,
    Qual,
    Aux,
}

impl BDSection {
    #[inline(always)]
    fn mk_flag(self) -> u8 {
        1 << (self as u8)
    }
}
#[derive(Default, Debug, Copy, Clone)]
pub struct BDMask {
    inner: u8,
}

impl BDMask {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn is_set(&self, s: BDSection) -> bool {
        self.inner & s.mk_flag() != 0
    }

    #[inline]
    pub fn is_seq_qual_set(&self) -> bool {
        self.inner & 0xc != 0
    }
    
    #[inline]
    pub fn set(&mut self, s: BDSection) {
        self.inner |= s.mk_flag()
    }

    #[inline]
    pub fn clear(&mut self, s: BDSection) {
        self.inner &= !s.mk_flag()
    }

    #[inline]
    pub(super) fn truncate(&mut self, s: BDSection) {
        self.inner &= s.mk_flag() - 1
    }

    /// Check that the obligatory sections (QNAME, CIGAR, SEQ, QUAL) have been done
    #[inline]
    pub(super) fn is_complete(&self) -> bool {
        self.inner & 0xf == 0xf
    }
    
    /// Compares a BDSection against the mask to see if it is greater than, equal to or less than the
    /// largest numbered section currently completed. This allows us to check whether we are appending
    /// to the data segment or inserting in the middle.
    ///
    /// i.e., if self.inner == 5, the largest numbered completed section is Seq (2). If s == Qual then
    /// this is 3 which is greater than 2, while if s = QName (0) then this is less than.
    #[inline]
    pub(super) fn compare(&self, s: BDSection) -> Ordering {
        if self.inner == 0 {
            std::cmp::Ordering::Greater
        } else {
            (s as u8).cmp(&(self.inner.ilog2() as u8))
        }
    }
}
