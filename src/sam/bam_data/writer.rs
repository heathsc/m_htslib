use std::io::{self, ErrorKind, Seek, SeekFrom, Write};

use crate::{SamError, base::Base, kstring::MString, sam::{CigarElem, record::bam1::aux::parse_aux_tag}};

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
        self.ms().write(buf)
    }
    
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.ms().write_all(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.ms().flush()
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
    fn ms(&mut self) -> &mut MString {
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
    
    pub fn aux_writer(self) -> Result<BDAuxWriter<'a>, SamError> {
        if matches!(self.bd.section, Some(BDSection::Aux)) {
            self.bd.hash.as_mut().unwrap().clear();
            let pos = if self.state.use_tmp { self.bd.data.len() as u32 } else { 0 };

            Ok(BDAuxWriter { inner: self, start_pos: pos, end_pos: pos })
        } else {
            Err(SamError::IllegalUseOfAuxWriter)
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

            let ks = self.inner.ms();
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
    pub fn write_elems(&mut self, seq: &[CigarElem]) -> Result<(), SamError> {
        let ks = self.inner.ms();
        for e in seq {
            ks.putsn(&e.to_le_bytes())
        }
        Ok(())
    }

    pub fn write_cigar(mut self, mut s: &[u8]) -> Result<(), SamError> {
        let ks = self.inner.ms();
        while !s.is_empty() {
            let (e, t) = CigarElem::parse(s)?;
            ks.putsn(&e.to_le_bytes());
            s = t;
        }
        Ok(())
    }
}

pub struct BDAuxWriter<'a> {
    inner: BDWriter<'a>,
    start_pos: u32,
    end_pos: u32,
}

impl Write for BDAuxWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }
    
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.inner.write_all(buf)
    }
    
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
} 

impl Seek for BDAuxWriter<'_> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        
        // Can't use the Self::ms() function here or we fall foul of the borrow checker...
        let ms = if !self.inner.state.use_tmp {
            &mut self.inner.bd.data
        } else {
            &mut self.inner.bd.tmp_data
        };

        let x = ms.len() as u32;
        let st = self.start_pos;
        if x > self.end_pos {
            self.end_pos = x
        }

        match pos {
            SeekFrom::Start(off) => {
                let new_pos = st + off as u32;
                if new_pos > self.end_pos {
                    Err(io::Error::new(ErrorKind::InvalidInput, "Seek past end"))
                } else {
                    unsafe { ms.set_len(new_pos as usize) }
                    Ok(off)
                }
            }
            SeekFrom::Current(off) => {
                eprintln!("Seek from current: {off}");
                let pos = ms.len() as i64;
                let new_pos = pos.checked_add(off).unwrap_or(-1);
                if new_pos < st as i64 || new_pos as u32 > self.end_pos {
                    Err(io::Error::new(ErrorKind::InvalidInput, "Illegal seek"))
                } else {
                    unsafe { ms.set_len(new_pos as usize) }
                    Ok(new_pos as u64 - st as u64)
                }
            }
            SeekFrom::End(off) => {
                eprintln!("Seek from end: {off}");
                let pos = self.end_pos as i64;
                let new_pos = pos.checked_add(off).unwrap_or(-1);
                if new_pos < st as i64 || new_pos as u32 > self.end_pos {
                    Err(io::Error::new(ErrorKind::InvalidInput, "Illegal seek"))
                } else {
                    unsafe { ms.set_len(new_pos as usize) }
                    Ok(new_pos as u64 - st as u64)
                }
            }
        }
    }
    
    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(self.inner.ms().len() as u64 - self.start_pos as u64)
    }
    
}

impl BDAuxWriter<'_> {
    pub fn write_aux_tag(&mut self, s: &[u8]) -> Result<(), SamError> {
        
        // We take the hash to avoid borrow checker problems
        let mut hash = self.inner.bd.hash.take().unwrap();
        
        // Parse tag
        let res = parse_aux_tag(self, s, &mut hash).map_err(SamError::AuxError);
        
        // Make sure we put hash back before we return, even if there was an error!
        self.inner.bd.hash = Some(hash);
        
        res
    }
    
    pub fn write_aux(mut self, s: &[u8]) -> Result<(), SamError> {
        for tag in s.split(|c| *c == b'\t') {
            self.write_aux_tag(tag)?
        }
        Ok(())
    }
}
