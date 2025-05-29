use std::{cmp::Ordering, collections::HashSet};

use super::{BDMask, BDSection, BDState, BDWriter, BamData};
use crate::{SamError, kstring::MString};

impl Default for BamData {
    fn default() -> Self {
        Self {
            state: BDState::default(),
            data: MString::default(),
            tmp_data: MString::default(),
            mask: BDMask::default(),
            section: None,
            last_error: None,
            hash: Some(HashSet::new()),
        }
    }
}

impl BamData {
    pub fn writer(&mut self, section: BDSection) -> BDWriter {
        assert!(self.section.is_none());
        self.section = Some(section);
        match self.mask.compare(section) {
            // Append to end of data segment
            Ordering::Greater => BDWriter::new(self, false),

            // Inserting in the middle of data segment
            Ordering::Less => {
                self.tmp_data.clear();
                BDWriter::new(self, true)
            }

            // Replacing latest section
            Ordering::Equal => {
                self.truncate(section);
                BDWriter::new(self, false)
            }
        }
    }

    #[inline]
    pub fn truncate(&mut self, section: BDSection) {
        let off = self.state.offset(section);
        self.data.truncate(off);
        self.mask.truncate(section);
    }

    pub fn offset_length(&self, section: BDSection) -> (usize, usize) {
        let off = self.state.offset(section);
        let len = match section {
            BDSection::QName => self.state.qname_len as usize,
            BDSection::Cigar => (self.state.n_cigar_elem << 2) as usize,
            BDSection::Seq => ((self.state.seq_len + 1) >> 1) as usize,
            BDSection::Qual => self.state.seq_len as usize,
            BDSection::Aux => self.data.len() - off,
        };

        (off, len)
    }

    #[inline]
    pub fn last_error(&self) -> Option<&SamError> {
        self.last_error.as_ref()
    }

    #[inline]
    pub fn clear_error(&mut self) -> Option<SamError> {
        self.last_error.take()
    }
}
