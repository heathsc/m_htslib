use std::ffi::CStr;

use crate::{hts::HtsPos, SamError};

use super::*;

impl bam1_t {
    pub fn end_pos(&self) -> HtsPos {
        unsafe { bam_endpos(self) }
    }
    
    pub fn set_query_name(&mut self, qname: &CStr) -> Result<(), SamError> {
        match unsafe { bam_set_qname(self, qname.as_ptr()) } {
            0 => Ok(()),
            _ => Err(SamError::SetQnameFailed)
        }
    }
    
    pub fn sam_parse(&mut self, kstring: &mut KString, hdr: &mut SamHdrRaw) -> Result<(), SamError> {
        match unsafe { sam_parse1(kstring, hdr, self) } {
            0 => Ok(()),
            _ => Err(SamError::ParseSamRecordFailed),
        }
    }
}