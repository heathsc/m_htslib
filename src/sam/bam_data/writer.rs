use std::io::Write;

use crate::{SamError, base::Base, kstring::KString, sam::CigarElem};

use super::{BDSection, BamData};

pub struct BDWriter<'a> {
    state: BDWriterState,
    bd: &'a mut BamData,
}

#[derive(Default, Debug)]
pub struct BDWriterState {
    use_tmp: bool,
    reported_size: bool,
    size: usize,
}

impl BDWriterState {
    #[inline]
    fn set_size(&mut self, size: usize) {
        self.size = size;
        self.reported_size = true;
    }

    #[inline]
    pub(super) fn use_tmp(&self) -> bool {
        self.use_tmp
    }

    #[inline]
    pub(super) fn size(&self) -> Option<usize> {
        if self.reported_size {
            Some(self.size)
        } else {
            None
        }
    }
}

impl Drop for BDWriter<'_> {
    fn drop(&mut self) {
        self.bd.validate_section(&self.state);
    }
}

impl Write for BDWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.ks().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.ks().flush()
    }
}

impl<'a> BDWriter<'a> {
    pub fn new(bd: &'a mut BamData, use_tmp: bool) -> Self {
        let state = BDWriterState {
            use_tmp,
            ..Default::default()
        };
        Self { bd, state }
    }

    #[inline]
    fn ks(&mut self) -> &mut KString {
        if !self.state.use_tmp {
            &mut self.bd.data
        } else {
            &mut self.bd.tmp_data
        }
    }

    #[inline]
    pub fn set_seq_len(&mut self, size: usize) {
        self.state.set_size(size)
    }
}

impl<'a> BDWriter<'a> {
    pub fn seq_writer(self) -> Result<BDSeqWriter<'a>, SamError> {
        if matches!(self.bd.section, Some(BDSection::Seq)) {
            Ok(BDSeqWriter { inner: self })
        } else {
            Err(SamError::IllegalUseOfSeqWriter)
        }
    }

    pub fn cigar_writer(self) -> Result<BDCigarWriter<'a>, SamError> {
        if matches!(self.bd.section, Some(BDSection::Cigar)) {
            Ok(BDCigarWriter { inner: self })
        } else {
            Err(SamError::IllegalUseOfCigarWriter)
        }
    }
}

pub struct BDSeqWriter<'a> {
    inner: BDWriter<'a>,
}

impl BDSeqWriter<'_> {
    pub fn write_seq(mut self, seq: &[u8]) -> Result<(), SamError> {
        if !seq.is_empty() && seq != b"*" {
            let iter = seq.chunks_exact(2);
            let r = iter.remainder();

            let ks = self.inner.ks();
            // Pack sequence into nybbles
            for s in iter {
                let x = Base::from_u8(s[0]).combine(&Base::from_u8(s[1]));
                ks.putc(x)?;
            }

            // Do remaining base if seq len is odd
            if let Some(c) = r.first() {
                ks.putc(Base::from_u8(*c).as_n() << 4)?
            }

            self.inner.set_seq_len(seq.len());
        }
        Ok(())
    }
}

pub struct BDCigarWriter<'a> {
    inner: BDWriter<'a>,
}

impl BDCigarWriter<'_> {
    pub fn write_elems(mut self, seq: &[CigarElem]) -> Result<(), SamError> {
        let ks = self.inner.ks();
        for e in seq {
            ks.putsn(&e.to_le_bytes())
        }
        Ok(())
    }
    
    pub fn write_cigar(mut self, mut s: &[u8]) -> Result<(), SamError> {
        let ks = self.inner.ks();
        while !s.is_empty() {
            let (e, t) = CigarElem::parse(s)?;
            ks.putsn(&e.to_le_bytes());
            s = t;
        }
        Ok(())
    }
}
