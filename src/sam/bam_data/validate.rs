use crate::{
    SamError,
    kstring::KString,
    sam::{
        Cigar, CigarElem, cigar_validate::valid_elem_slice,
        record::bam1::aux_iter::validate_aux_slice,
    },
};

use super::{BDSection, BDWriterState, BamData};

const ZEROS: [u8; 4] = [0, 0, 0, 0];

impl BamData {
    pub(super) fn validate_section(&mut self, state: &BDWriterState) {
        if let Some(s) = self.section.take() {
            let tmp_data = state.use_tmp();

            let res = match s {
                BDSection::QName => self.validate_qname(tmp_data),
                BDSection::Cigar => self.validate_cigar(tmp_data),
                BDSection::Seq => self.validate_seq(tmp_data, state.size()),
                BDSection::Qual => self.validate_qual(tmp_data, state.size()),
                BDSection::Aux => self.validate_aux(tmp_data),
            };

            match res {
                Ok(()) => {
                    if tmp_data {
                        self.insert_tmp_data(s);
                    }
                    self.mask.set(s)
                }
                Err(e) => {
                    if !tmp_data {
                        let off = self.state.offset(s);
                        self.data.truncate(off);
                    }
                    self.last_error = Some(e);
                    self.mask.clear(s);
                }
            }
        }
    }

    /// Check that the sections are consistent with each other, and that the BAM data segment is minimally complete
    /// (i.e., that the obligitory parts are there)
    pub fn validate(&mut self) -> Result<(), SamError> {
        // This should be assured by the borrow checkcer...
        assert!(self.section.is_none());
        if let Some(e) = self.last_error.take() {
            Err(e)
        } else if !self.mask.is_complete() {
            Err(SamError::IncompleteDataSegment)
        } else {
            Ok(())
        }
    }

    /// self.tmp_data contains validated data for one section, which needs to be inserted into the
    /// self.data in the correct place
    fn insert_tmp_data(&mut self, s: BDSection) {
        assert!(s != BDSection::Aux);
        let (off, len) = self.offset_length(s);
        let new_len = self.tmp_data.len();
        if new_len > len {
            self.data.expand(new_len - len);
        };
        let ptr = self.data.as_ptr_mut();
        assert!(!ptr.is_null());
        let sz = len.abs_diff(new_len);
        if sz > 0 {
            unsafe { std::ptr::copy(ptr.add(off + len), ptr.add(off + new_len), sz) }
        }
        if new_len > 0 {
            let tptr = self.tmp_data.as_ptr();
            assert!(!tptr.is_null());
            unsafe { std::ptr::copy(tptr, ptr.add(off), new_len) }
        }
    }

    fn get_data_len(&self, tmp_data: bool) -> usize {
        if tmp_data {
            self.tmp_data.len()
        } else {
            self.data.len()
        }
    }

    fn get_kstring_mut(&mut self, tmp_data: bool) -> &mut KString {
        if !tmp_data {
            &mut self.data
        } else {
            &mut self.tmp_data
        }
    }

    fn get_kstring(&self, tmp_data: bool) -> &KString {
        if !tmp_data {
            &self.data
        } else {
            &self.tmp_data
        }
    }

    fn get_test_seq_len(&self, size: Option<usize>, seq_empty: bool) -> Result<usize, SamError> {
        eprintln!("EEEEK {:?} {} {:?}", size, seq_empty, self.get_cigar_qlen());
        match (size, self.get_qual_len(), self.get_cigar_qlen()) {
            // We have a non zero length from both the Cigar and Qual fields and they are not equal
            // This shouldn't happen as it should have been caught before
            (_, Some(b), Some(c)) if b != c && b > 0 && c > 0 => Err(SamError::SeqCigarMismatch),
            // We have the given length and any one of the other two estimates and they don't match
            (Some(a), Some(b), _) | (Some(a), None, Some(b)) if a != b && b > 0 => {
                Err(SamError::SeqLenMismatch)
            }
            // It is an error if Seq is empty and Qual is non-empty
            (_, Some(a), _) if a > 0 && seq_empty => Err(SamError::SeqCigarMismatch),
            // We have no valid estimates for the seq length
            (None, None, None) | (None, Some(0), None) | (None, None, Some(0)) => {
                Err(SamError::SeqLenNotSet)
            }
            // Otherwise all is good, and we can use any of the estimates as we know that, where present, they are equal
            (Some(a), _, _) | (None, Some(a), _) | (None, None, Some(a)) => Ok(a),
        }
    }

    fn get_test_qual_len(&self, size: Option<usize>, qlen: usize) -> Result<usize, SamError> {
        eprintln!(
            "ACK {:?} {qlen} {:?} {:?} {}",
            size,
            self.get_seq_len(),
            self.get_cigar_qlen(),
            self.state.seq_len,
        );
        match (size, self.get_seq_len(), self.get_cigar_qlen()) {
            // We have a non zero length from both the Cigar and Seq fields and they are not equal
            // This shouldn't happen as it should have been caught before
            (_, Some(b), Some(c)) if b != c && b > 0 && c > 0 => Err(SamError::SeqCigarMismatch),
            // We have the given length and any one of the other two estimates and they don't match
            (Some(a), Some(b), _) | (Some(a), None, Some(b)) if a != b && b > 0 => {
                Err(SamError::SeqLenMismatch)
            }
            (Some(a), _, _) if a != qlen && qlen > 0 => Err(SamError::SeqLenMismatch),
            // If Seq is empty than Qual should also be empty
            (_, Some(a), _) => {
                if a != qlen && qlen > 0 {
                    Err(SamError::SeqQualMismatch)
                } else {
                    Ok(a)
                }
            }
            (_, _, Some(a)) if a != qlen && a > 0 && qlen > 0 => Err(SamError::SeqCigarMismatch),
            // Otherwise return observed qual len
            _ => Ok(qlen),
        }
    }

    fn validate_seq(&mut self, tmp_data: bool, size: Option<usize>) -> Result<(), SamError> {
        let off = self.state.seq_offset();
        let n_bytes = self.get_data_len(tmp_data).checked_sub(off).unwrap();
        assert!(n_bytes <= i32::MAX as usize);

        eprintln!("OOOOK! {n_bytes} {:?}", size);

        if n_bytes == 0 {
            self.state.seq_len = 0;
            Ok(())
        } else {
            let r = self.get_test_seq_len(size, n_bytes == 0);
            eprintln!("Aha! {:?}", r);
            let seq_len = r?;
            eprintln!("seq_len = {seq_len}");
            
            // Check the number of bytes is consistent with the expected number of bases
            if n_bytes != (seq_len + 1) >> 1 {
                Err(SamError::SeqLenMismatch)
            } else {
                if self.state.seq_len == 0 && self.mask.is_set(BDSection::Qual) {
                    self.fill_dummy_qual(tmp_data, seq_len);
                }
                self.state.seq_len = seq_len as i32;
                Ok(())
            }
        }
    }

    fn fill_dummy_qual(&mut self, tmp_data: bool, l: usize) {
        // In this case we will fill in a dummy qual string, setting all values to 255
        let ks = self.get_kstring_mut(tmp_data);
        for _ in 0..l {
            ks.putc(255).unwrap()
        }
    }

    fn validate_qual(&mut self, tmp_data: bool, size: Option<usize>) -> Result<(), SamError> {
        let off = self.state.qual_offset();
        let qlen = self.get_data_len(tmp_data).checked_sub(off).unwrap();
        assert!(qlen <= i32::MAX as usize);

        let seq_len = self.get_test_qual_len(size, qlen)?;
        if seq_len != qlen {
            // We should only be here if qlen == 0 and Seq was non empty.
            assert_eq!(qlen, 0);

            // In this case we will fill in a dummy qual string, setting all values to 255
            self.fill_dummy_qual(tmp_data, seq_len);
        } else {
            self.state.seq_len = qlen as i32
        }

        Ok(())
    }

    fn validate_aux(&mut self, tmp_data: bool) -> Result<(), SamError> {
        let off = self.state.aux_offset();

        // We can't use self.get_data or self.get_kstring as this annoys the
        // borrow checker...
        let ks = if tmp_data { &self.tmp_data } else { &self.data };

        let s = &ks.as_slice()[off..];
        validate_aux_slice(s, &mut self.hash)?;
        Ok(())
    }

    fn get_cigar_qlen(&self) -> Option<usize> {
        if self.mask.is_set(BDSection::Cigar) && self.state.n_cigar_elem > 0 {
            let off = self.state.cigar_offset();
            let ks = self.get_kstring(false);
            let s = get_elem_slice(ks, off, self.state.n_cigar_elem as usize);

            // If the cigar data are here then they have already been validated
            let cigar = unsafe { Cigar::from_elems_unchecked(s) };

            Some(cigar.query_len() as usize)
        } else {
            None
        }
    }

    fn get_sq_len(&self, s: BDSection) -> Option<usize> {
        if self.mask.is_set(s) {
            Some(self.state.seq_len as usize)
        } else {
            None
        }
    }

    #[inline]
    fn get_qual_len(&self) -> Option<usize> {
        self.get_sq_len(BDSection::Qual)
    }

    #[inline]
    fn get_seq_len(&self) -> Option<usize> {
        self.get_sq_len(BDSection::Seq)
    }

    fn validate_cigar(&mut self, tmp_data: bool) -> Result<(), SamError> {
        let off = self.state.cigar_offset();
        let ks = self.get_kstring(tmp_data);
        
        let cigar_len = ks.len() - off;
        if cigar_len & 3 != 0 {
            Err(SamError::CigarLengthNotMul4)
        } else {
            self.state.n_cigar_elem = if cigar_len > 0 {
                let l = cigar_len >> 2;
                if l > u32::MAX as usize {
                    return Err(SamError::TooManyCigarElem);
                }
                let s = get_elem_slice(ks, off, l);
                valid_elem_slice(s)?;
                if self.mask.is_seq_qual_set() && self.state.seq_len > 0 {
                    let c = unsafe { Cigar::from_elems_unchecked(s) };
                    if c.query_len() != self.state.seq_len as u32 {
                        return Err(SamError::SeqCigarMismatch);
                    }
                }
                l as u32
            } else {
                0
            };
            Ok(())
        }
    }

    fn validate_qname(&mut self, tmp_data: bool) -> Result<(), SamError> {
        let ks = self.get_kstring_mut(tmp_data);
        let s = ks.as_slice();

        // Using KString we are assured that the name is null terminated, so we just need to
        // check for internal nulls or other illegal characters.
        //
        // The SAM spec states that the name can be any ascii graphic apart from @, and
        // the length should be from 1-254 (not including the null)

        if s.is_empty() {
            Err(SamError::EmptyQueryName)
        } else {
            let l = s.len();
            if s.iter().any(|c| !c.is_ascii_graphic() || *c == b'@') {
                Err(SamError::IllegalQueryNameChar)
            } else if l > 254 {
                Err(SamError::QueryNameTooLong)
            } else {
                // Add terminating nul + extra nuls to force alignment to 4 bytes,
                let n_extra_nuls = 3 - (l & 3);
                ks.putsn(&ZEROS[..n_extra_nuls + 1]);
                self.state.extra_nul = n_extra_nuls as u8;
                self.state.qname_len = (l + 1 + n_extra_nuls) as u16;
                Ok(())
            }
        }
    }
}

fn get_elem_slice(ks: &KString, off: usize, l: usize) -> &[CigarElem] {
    let p = ks.as_ptr();
    if p.is_null() || l == 0 {
        &[]
    } else {
        unsafe {
            let ptr = p.add(off);
            assert_eq!(
                ptr.align_offset(4),
                0,
                "Cigar storage not aligned - Bam record corrupt"
            );
            std::slice::from_raw_parts(ptr.cast::<CigarElem>(), l)
        }
    }
}
