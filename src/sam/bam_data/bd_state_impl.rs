use super::{BDState, BDSection};

impl BDState {
    #[inline]
    pub(super) fn cigar_offset(&self) -> usize {
        assert_eq!(self.qname_len & 3, 0);
        self.qname_len as usize
    }

    #[inline]
    pub(super) fn seq_offset(&self) -> usize {
        self.cigar_offset() + self.n_cigar_elem as usize * std::mem::size_of::<u32>()
    }

    #[inline]
    pub(super) fn qual_offset(&self) -> usize {
        self.seq_offset() + ((self.seq_len + 1) >> 1) as usize
    }

    #[inline]
    pub(super) fn aux_offset(&self) -> usize {
        self.qual_offset() + self.seq_len as usize
    }

    pub(super) fn offset(&self, s: BDSection) -> usize {
        match s {
            BDSection::QName => 0,
            BDSection::Cigar => self.cigar_offset(),
            BDSection::Seq => self.seq_offset(),
            BDSection::Qual => self.qual_offset(),
            BDSection::Aux => self.aux_offset(),
        }
    }
}