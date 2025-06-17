use std::ops::DerefMut;

use libc::c_int;

use crate::{
    SamError,
    hts::{HtsFile, HtsFileRaw, traits::ReadRec},
    sam::{SamHdr, SamHdrRaw},
};

use super::{BamRec, bam1::bam1_t};

#[link(name = "hts")]
unsafe extern "C" {
    fn sam_read1(fp_: *mut HtsFileRaw, hd_: *mut SamHdrRaw, b_: *mut bam1_t) -> c_int;
}

pub struct SamReader<'a: 'b, 'b, 'c> {
    hts_file: &'b mut HtsFile<'a>,
    hdr: &'c SamHdr,
}

impl<'a, 'b, 'c> SamReader<'a, 'b, 'c> {
    pub fn new(hts_file: &'b mut HtsFile<'a>, hdr: &'c SamHdr) -> Self {
        Self { hts_file, hdr }
    }
}

impl ReadRec for SamReader<'_, '_, '_> {
    type Err = SamError;
    type Rec = BamRec;

    fn read_rec<'a>(&mut self, rec: &'a mut Self::Rec) -> Result<Option<&'a Self::Rec>, Self::Err> {
        let (_g, hdr) = self.hdr.get_mut();

        match unsafe { sam_read1(self.hts_file.deref_mut(), hdr, rec.as_mut_ptr()) } {
            0.. => Ok(Some(rec)),
            -1 => Ok(None), // EOF
            e => Err(SamError::SamReadError(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused)]

    use super::*;

    use crate::hts::HtsFile;

    #[test]
    fn test_read_sam() {
        let mut h =
            HtsFile::open(c"test/realn01.sam", c"r").expect("Failed to read test/realn01.sam");
        let hdr = SamHdr::read(&mut h).expect("Failed to read header");
        let mut rec = BamRec::new();
        let mut reader = SamReader::new(&mut h, &hdr);
        
        let mut n = 0;
        while let Some(r) = reader.read_rec(&mut rec).unwrap() {
            eprintln!("{:?}", r.qname());
            n+=1;
        }
        
        assert_eq!(n, 4);
    }    #[test]
    fn test_read_cram() {
        let mut h =
            HtsFile::open(c"test/test_input_1_a.cram", c"r").expect("Failed to read test/test_input_1_a.cram");
        let hdr = SamHdr::read(&mut h).expect("Failed to read header");
        let mut rec = BamRec::new();
        let mut reader = SamReader::new(&mut h, &hdr);
        
        let mut n = 0;
        while let Some(r) = reader.read_rec(&mut rec).unwrap() {
            eprintln!("{:?}", r.qname());
            n+=1;
        }
        
        assert_eq!(n, 15);
    }
}
