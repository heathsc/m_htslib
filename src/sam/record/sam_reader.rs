use std::{
    borrow::Borrow,
    ops::{Deref, DerefMut},
    ptr::null_mut,
};

use libc::{c_char, c_int, c_void};

use crate::{
    HtsError, SamError,
    hts::{
        HtsFile, HtsFileRaw, HtsIdx, HtsIdxRaw, HtsPos, HtsRegion, HtslibRegion,
        hts_itr::{HtsItr, HtsItrRaw, HtsRegionIter, HtsRegionsIter, hts_itr_next},
        traits::{HdrType, IdMap, ReadRec, ReadRecIter, SeqId},
    },
    sam::{SamHdr, SamHdrRaw},
};

use super::{BamRec, bam1::bam1_t};

#[link(name = "hts")]
unsafe extern "C" {
    fn sam_read1(fp_: *mut HtsFileRaw, hd_: *mut SamHdrRaw, b_: *mut bam1_t) -> c_int;
    unsafe fn sam_itr_queryi(
        idx: *const HtsIdxRaw,
        tid: c_int,
        beg: HtsPos,
        end: HtsPos,
    ) -> *mut HtsItrRaw;
    unsafe fn sam_index_load(fp_: *mut HtsFileRaw, fn_: *const c_char) -> *mut HtsIdxRaw;
}

pub struct SamReader<'a: 'b, 'b, 'c> {
    hts_file: &'b mut HtsFile<'a>,
    hdr: &'c SamHdr,
    idx: Option<HtsIdx>,
}

impl<'a, 'b, 'c> SamReader<'a, 'b, 'c> {
    pub fn new(hts_file: &'b mut HtsFile<'a>, hdr: &'c SamHdr) -> Self {
        Self {
            hts_file,
            hdr,
            idx: None,
        }
    }
}

impl SamReader<'_, '_, '_> {
    pub fn region_iter(mut self, region: &HtsRegion) -> Result<impl ReadRec<Rec = BamRec, Err = SamError> + IdMap, SamError> {
        self.load_idx()?;
        let idx = self.idx.take().unwrap();
        let reg = region.make_htslib_region(self.hdr).expect("Invalid region");
        let f = move |r: &HtslibRegion| -> Option<HtsItr> {
            HtsItr::make(unsafe { sam_itr_queryi(idx.deref(), r.tid(), r.start(), r.end()) })
        };
        Ok(HtsRegionIter::make_region_iter(reg, f, self))
    }
    
    pub fn regions_iter<'a, I, T>(
        mut self,
        regions: I,
    ) -> Result<impl ReadRec<Rec = BamRec, Err = SamError> + IdMap, SamError> 
    where 
        I: Iterator<Item = T>,
        T: Borrow<HtsRegion<'a>>,
    {
        self.load_idx()?;
        let idx = self.idx.take().unwrap();
        let reg_iter = regions.map(|r| r.borrow().make_htslib_region(self.hdr).expect("Invalid region"));
        let f = move |r: &HtslibRegion| -> Option<HtsItr> {
            HtsItr::make(unsafe { sam_itr_queryi(idx.deref(), r.tid(), r.start(), r.end()) })
        };
        
       Ok(HtsRegionsIter::make_regions_iter(reg_iter, f, self))
    }
    
    pub fn load_idx(&mut self) -> Result<(), SamError> {
        if self.idx.is_none() {
            let hts_raw = self.hts_file.deref_mut();

            let fname = hts_raw.file_name_ptr();
            let idx_ptr = unsafe { sam_index_load(hts_raw, fname) };
            let idx = HtsIdx::mk_hts_idx(idx_ptr, HtsError::IOError)
                .map_err(|_| SamError::OperationFailed)?;
            self.idx = Some(idx);
        }
        Ok(())
    }
}

impl ReadRec for SamReader<'_, '_, '_> {
    type Err = SamError;
    type Rec = BamRec;

    fn read_rec(&mut self, rec: &mut Self::Rec) -> Result<Option<()>, Self::Err> {
        let mut g = self.hdr.write_guard();

        match unsafe { sam_read1(self.hts_file.deref_mut(), g.as_ptr_mut(), rec.as_mut_ptr()) } {
            0.. => Ok(Some(())),
            -1 => Ok(None), // EOF
            e => Err(SamError::SamReadError(e)),
        }
    }
}

impl ReadRecIter for SamReader<'_, '_, '_> {
    fn read_rec_iter(
        &mut self,
        itr: &mut HtsItr,
        rec: &mut Self::Rec,
    ) -> Result<Option<()>, Self::Err> {
        let bgzf = self
            .hts_file
            .bgzf_desc()
            .map(|p| p.as_ptr())
            .unwrap_or_else(null_mut);

        match unsafe {
            hts_itr_next(
                bgzf,
                itr.deref_mut(),
                rec.as_mut_ptr() as *mut c_void,
                self.hts_file.deref_mut() as *mut HtsFileRaw as *mut c_void,
            )
        } {
            0.. => Ok(Some(())),
            -1 => Ok(None),
            e => Err(SamError::SamReadError(e)),
        }
    }
}

impl HdrType for SamReader<'_, '_, '_> {
    fn hdr_type(&self) -> crate::hts::traits::HtsHdrType {
        self.hdr.hdr_type()
    }
}

impl SeqId for SamReader<'_, '_, '_> {
    fn seq_id(&self, s: &std::ffi::CStr) -> Option<usize> {
        self.hdr.seq_id(s)
    }
}

impl IdMap for SamReader<'_, '_, '_> {
    fn seq_len(&self, i: usize) -> Option<usize> {
        self.hdr.seq_len(i)
    }

    fn seq_name(&self, i: usize) -> Option<&std::ffi::CStr> {
        self.hdr.seq_name(i)
    }

    fn num_seqs(&self) -> usize {
        self.hdr.num_seqs()
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused)]

    use super::*;

    use crate::{
        hts::{HtsFile, traits::IdMap},
        region::reg::{Reg, Region},
    };

    #[test]
    fn test_read_sam() {
        let mut h =
            HtsFile::open(c"test/realn01.sam", c"r").expect("Failed to read test/realn01.sam");
        let hdr = SamHdr::read(&mut h).expect("Failed to read header");
        let mut rec = BamRec::new();
        let mut reader = SamReader::new(&mut h, &hdr);

        let mut n = 0;
        while reader.read_rec(&mut rec).unwrap().is_some() {
            eprintln!("{:?}", rec.qname());
            n += 1;
        }

        assert_eq!(n, 4);
    }
    #[test]
    fn test_read_cram() {
        let mut h = HtsFile::open(c"test/test_input_1_a.cram", c"r")
            .expect("Failed to read test/test_input_1_a.cram");
        let hdr = SamHdr::read(&mut h).expect("Failed to read header");
        let mut rec = BamRec::new();
        let mut reader = SamReader::new(&mut h, &hdr);

        let mut n = 0;
        while reader.read_rec(&mut rec).unwrap().is_some() {
            let ctg = rec.tid().and_then(|i| reader.seq_name(i));
            eprintln!("{:?} {:?} {:?}", rec.qname(), ctg, rec.pos());
            n += 1;
        }

        assert_eq!(n, 15);
    }

    #[test]
    fn test_read_cram_iter() {
        let mut h = HtsFile::open(c"test/test_input_1_a.cram", c"r")
            .expect("Failed to read test/test_input_1_a.cram");
        let hdr = SamHdr::read(&mut h).expect("Failed to read header");

        let mut rec = BamRec::new();
        let mut reader = SamReader::new(&mut h, &hdr);
        let reg = Reg::from_u8_slice(b"ref2:25-").unwrap();
        let region = Region::from_reg(&reg);
        let hreg: HtsRegion = HtsRegion::from(&region);

        let mut itr = reader.region_iter(&hreg).unwrap();

        let mut n = 0;

        while itr.read_rec(&mut rec).unwrap().is_some() {
            let ctg = rec.tid().and_then(|i| itr.seq_name(i));
            eprintln!("{:?} {:?} {:?}", rec.qname(), ctg, rec.pos());
            n += 1;
        }

        assert_eq!(n, 4);
    }
    
    #[test]
    fn test_read_cram_multi_iter() {
        let mut h = HtsFile::open(c"test/test_input_1_a.cram", c"r")
            .expect("Failed to read test/test_input_1_a.cram");
        let hdr = SamHdr::read(&mut h).expect("Failed to read header");

        let mut rec = BamRec::new();
        let mut reader = SamReader::new(&mut h, &hdr);
        let reg = Reg::from_u8_slice(b"ref1").unwrap();
        let region1 = Region::from_reg(&reg);
        let reg = Reg::from_u8_slice(b"ref2:25-").unwrap();
        let region2 = Region::from_reg(&reg);
        let hreg1: HtsRegion = HtsRegion::from(&region1);
        let hreg2: HtsRegion = HtsRegion::from(&region2);
        let hregs = [hreg1, hreg2];

        let mut itr = reader.regions_iter(hregs.iter()).unwrap();

        let mut n = 0;

        while itr.read_rec(&mut rec).unwrap().is_some() {
            let ctg = rec.tid().and_then(|i| itr.seq_name(i));
            eprintln!("{:?} {:?} {:?}", rec.qname(), ctg, rec.pos());
            n += 1;
        }

        assert_eq!(n, 10);
    }
}
