use std::{
    ops::DerefMut,
};

use libc::c_int;

use crate::{
    SamError,
    hts::{
        HtsFile, HtsFileRaw,
        traits::{HdrType, IdMap, WriteRec, SeqId},
    },
    sam::{SamHdr, SamHdrRaw},
};

use super::{BamRec, bam1::bam1_t};

#[link(name = "hts")]
unsafe extern "C" {
    fn sam_write1(fp_: *mut HtsFileRaw, hd_: *mut SamHdrRaw, b_: *mut bam1_t) -> c_int;
}

pub struct SamWriter<'a: 'b, 'b, 'c> {
    hts_file: &'b mut HtsFile<'a>,
    hdr: &'c SamHdr,
}

impl<'a, 'b, 'c> SamWriter<'a, 'b, 'c> {
    pub fn new(hts_file: &'b mut HtsFile<'a>, hdr: &'c SamHdr) -> Self {
        Self {
            hts_file,
            hdr,
        }
    }
}

impl WriteRec for SamWriter<'_, '_, '_> {
    type Err = SamError;
    type Rec = BamRec;

    fn write_rec(&mut self, rec: &mut Self::Rec) -> Result<Option<()>, Self::Err> {
        let mut g = self.hdr.write_guard();

        match unsafe { sam_write1(self.hts_file.deref_mut(), g.as_ptr_mut(), rec.as_mut_ptr()) } {
            0.. => Ok(Some(())),
            -1 => Ok(None), // EOF
            e => Err(SamError::SamReadError(e)),
        }
    }
}

impl HdrType for SamWriter<'_, '_, '_> {
    fn hdr_type(&self) -> crate::hts::traits::HtsHdrType {
        self.hdr.hdr_type()
    }
}

impl SeqId for SamWriter<'_, '_, '_> {
    fn seq_id(&self, s: &std::ffi::CStr) -> Option<usize> {
        self.hdr.seq_id(s)
    }
}

impl IdMap for SamWriter<'_, '_, '_> {
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
