use std::{
    io::{self, Write},
    marker::PhantomData,
    ptr::copy_nonoverlapping,
};

use libc::{c_int, c_void, realloc};

use crate::{
    SamError,
    sam::{CigarElem, cigar, cigar_validate::valid_elem_slice},
};

const ZEROS: [u8; 3] = [0, 0, 0];

#[derive(Debug)]
pub struct BamData {
    data_size: u32,
    state: BDState,
    data: *mut u8,
    in_progress: bool,
    marker: PhantomData<u8>,
}

impl Default for BamData {
    fn default() -> Self {
        Self {
            data_size: 0,
            state: BDState::default(),
            data: std::ptr::null_mut(),
            in_progress: false,
            marker: PhantomData,
        }
    }
}

impl BamData {
    /// In common with standard rust memory allocation, we panic if memory is not available
    /// or if allocation requested is too large
    fn realloc_data(&mut self, size: usize) {
        let s = crate::roundup(size);
        assert!(
            s <= c_int::MAX as usize,
            "Requested allocation size is too large for Bam Record"
        );
        let new_data = unsafe { realloc(self.data as *mut c_void, s) };
        assert!(!new_data.is_null(), "Out of memory");

        self.data = new_data as *mut u8;
        self.data_size = s as u32;

        if s < self.state.data_used as usize {
            // If we have reduced the data size below what was in use, then we can't trust anything
            // so we clear the record.
            self.state.clear();
        }
    }

    #[inline]
    fn reserve(&mut self, additional: usize) {
        let sz = (self.state.data_used as usize)
            .checked_add(additional)
            .expect("Allocation size too high");
        if sz > self.data_size as usize {
            self.realloc_data(sz)
        }
    }

    #[inline]
    fn get_data_slice(&self) -> &[u8] {
        unsafe { super::make_data_slice(self.data, 0, self.state.data_used as usize) }
    }

    #[inline]
    pub fn check(&mut self) -> Result<(), SamError> {
        self.check_internal(false)
    }

    /// # Safety
    ///
    /// Only perform minimal (cheap) validation.
    /// Complete checks that the [u8] slice provided by Write forms valid BAM structures are not performed.
    /// The caller is responsible to ensure that the bam record data segment is valid.
    #[inline]
    pub unsafe fn check_no_validate(&mut self) -> Result<(), SamError> {
        self.check_internal(true)
    }

    fn check_internal(&mut self, skip_validation: bool) -> Result<(), SamError> {
        if self.state.incomplete {
            self.state.stage = match self.state.stage {
                BDStage::QName => {
                    self.check_qname(skip_validation)?;
                    BDStage::Cigar
                }
                BDStage::Cigar => {
                    self.check_cigar(skip_validation)?;
                    BDStage::Seq
                }
                BDStage::Seq => {
                    self.check_seq()?;
                    BDStage::Qual
                }
                BDStage::Qual => {
                    self.check_qual()?;
                    BDStage::Aux
                }
                BDStage::Aux | BDStage::AuxAppend => {
                    if !skip_validation {
                        self.check_aux()?
                    }
                    BDStage::AuxAppend
                }
            };
            self.state.incomplete = false;
        }
        Ok(())
    }

    fn check_aux(&mut self) -> Result<(), SamError> {
        let off = self.state.aux_offset();
        let s = &self.get_data_slice()[off..];
        super::aux_iter::validate_aux_slice(s)?;
        Ok(())
    }

    fn check_seq(&mut self) -> Result<(), SamError> {
        let off = self.state.seq_offset();
        let s = self.get_data_slice();
        let seq = &s[off..];
        assert!(seq.len() <= i32::MAX as usize);

        // We can't get the exact equence length from the input because of the packing which means the LSB is unknown,
        // so we get the expected sequence length from the cigar.
        let cigar_elem =
            self.get_elem_slice(self.state.cigar_offset(), self.state.n_cigar_elem as usize);
        let cigar = unsafe { cigar::from_elems_unchecked(cigar_elem) };
        let seq_len = cigar.query_len() as usize;
        
        // Check if sequence length is comparitble with cigar
        // Note that sequence is packed 2 bases per byte
        if (seq_len + 1) >> 1 != seq.len() {
            Err(SamError::SeqCigarMismatch)
        } else {
            self.state.seq_len = seq_len as i32;
            Ok(())
        }
    }

    fn check_qual(&mut self) -> Result<(), SamError> {
        let off = self.state.qual_offset();
        let s = &self.get_data_slice()[off..];
        if s.len() != self.state.seq_len as usize {
            Err(SamError::SeqQualMismatch)
        } else {
            Ok(())
        }
    }

    fn get_elem_slice(&self, off: usize, l: usize) -> &[CigarElem] {
        if self.data.is_null() || l == 0 {
            &[]
        } else {
            unsafe {
                let ptr = self.data.add(off);
                assert_eq!(
                    ptr.align_offset(4),
                    0,
                    "Cigar storage not aligned - Bam record corrupt"
                );
                std::slice::from_raw_parts(ptr.cast::<CigarElem>(), l)
            }
        }
    }

    fn check_cigar(&mut self, skip_validation: bool) -> Result<(), SamError> {
        let off = self.state.cigar_offset();
        let cigar_len = self.state.data_used as usize - off;
        if cigar_len & 3 != 0 {
            Err(SamError::CigarLengthNotMul4)
        } else {
            self.state.n_cigar_elem = if !self.data.is_null() && cigar_len > 0 {
                let l = cigar_len >> 2;
                if l > u32::MAX as usize {
                    return Err(SamError::TooManyCigarElem);
                }
                let s = self.get_elem_slice(off, l);
                if !skip_validation {
                    valid_elem_slice(s)?;
                }
                l as u32
            } else {
                0
            };
            Ok(())
        }
    }

    fn check_qname(&mut self, skip_validation: bool) -> Result<(), SamError> {
        let s = self.get_data_slice();

        if skip_validation {
            // In the case of no validation, we just add the terminating nul if the last character is not a null
            if s.last().map(|c| *c == 0).unwrap_or(false) {
                let _ = self.write(&[0]).unwrap();
            }
        } else {
            // Check name is null terminated and contains no illegal characters
            match s.iter().position(|c| !c.is_ascii_graphic() || *c == b'@') {
                // Name is OK, but no null terminator found, so we add one
                None => {
                    let _ = self.write(&[0]).unwrap();
                }
                // Only non graphic character is the terminating zero
                Some(i) if i + 1 == s.len() && s[i] == 0 => {}
                _ => return Err(SamError::IllegalQueryNameChar),
            }
        }
        let l = self.state.data_used;

        if (2..=255).contains(&l) {
            // QName is valid, so add extranuls to force alignment to 4 bytes,
            // and sett tje lengths appropriately
            let l1 = (l + 3) & 3;
            let l2 = (l1 - l) as usize;
            let _ = self.write(&ZEROS[..l2]);
            self.state.data_used = l1;
            self.state.extra_nul = l2 as u8;
            Ok(())
        } else if l < 2 {
            // We only have the terminating null
            Err(SamError::EmptyQueryName)
        } else {
            // Sam specs restricts query name to 254 characters (without the terminating null)
            Err(SamError::QueryNameTooLong)
        }
    }
}

impl Write for BamData {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.in_progress = true;
        let sz = buf.len();
        if sz > 0 {
            self.reserve(sz);
            unsafe {
                copy_nonoverlapping(
                    buf.as_ptr(),
                    self.data.add(self.state.data_used as usize),
                    sz,
                );
            }
            self.state.data_used += sz as c_int;
        }
        Ok(sz)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.check().map_err(io::Error::other)
    }
}

#[derive(Debug, Default, Copy, Clone)]
struct BDState {
    data_used: c_int,
    n_cigar_elem: u32,
    qname_len: u16,
    extra_nul: u8,
    stage: BDStage,
    incomplete: bool,
    seq_len: i32,
}

impl BDState {
    #[inline]
    fn clear(&mut self) {
        *self = Self::default()
    }

    #[inline]
    fn cigar_offset(&self) -> usize {
        assert_eq!(self.qname_len & 3, 0);
        self.qname_len as usize
    }

    #[inline]
    fn seq_offset(&self) -> usize {
        self.cigar_offset() + self.n_cigar_elem as usize * std::mem::size_of::<u32>()
    }

    #[inline]
    fn qual_offset(&self) -> usize {
        self.seq_offset() + self.seq_len as usize
    }

    #[inline]
    fn aux_offset(&self) -> usize {
        let i = self.seq_len as usize;
        self.seq_offset() + ((i + 1) >> 1) + i
    }

    fn truncate_to(&mut self, st: BDStage) {
        if st > self.stage {
            panic!("Illegal operation - cannot truncate forwards")
        }
        match st {
            BDStage::QName => self.clear(),
            BDStage::Cigar => {
                self.data_used = self.qname_len as c_int;
                self.n_cigar_elem = 0;
            }
            BDStage::Seq => self.data_used = self.seq_offset() as c_int,
            BDStage::Qual => self.data_used = self.qual_offset() as c_int,
            BDStage::Aux => self.data_used = self.aux_offset() as c_int,
            BDStage::AuxAppend => {}
        }
        self.stage = st;
        self.incomplete = false;
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum BDStage {
    #[default]
    QName,
    Cigar,
    Seq,
    Qual,
    Aux,
    AuxAppend,
}
