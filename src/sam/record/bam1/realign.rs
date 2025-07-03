use libc::{c_char, c_int};

use crate::{SamError, faidx::Sequence, hts::HtsPos, sam::BamRec};

use super::bam1_t;

pub const BAQ_APPLY: c_int = 1;
pub const BAQ_EXTEND: c_int = 2;
pub const BAQ_REDO: c_int = 4;

pub const BAQ_AUTO: c_int = 0;
pub const BAQ_ILLUMINA: c_int = 1 << 3;
pub const BAQ_PACBIOCCS: c_int = 2 << 3;
pub const BAQ_PACBIO: c_int = 3 << 3;
pub const BAQ_ONT: c_int = 4 << 3;
pub const BAQ_GENAPSYS: c_int = 5 << 3;

#[link(name = "hts")]
unsafe extern "C" {
    fn sam_prob_realn(b: *mut bam1_t, rf: *const c_char, rf_len: HtsPos, flags: c_int) -> c_int;
}

impl BamRec {
    pub fn realign(&mut self, seq: &Sequence, flags: c_int) -> Result<bool, SamError> {
        if self.is_mapped()
            && let Some(pos) = self.pos()
        {
            let endpos = self.endpos();
            let rf = seq.get_seq((pos + 1) as usize, (endpos + 1) as usize)?;
            if !rf.is_empty() {
                match unsafe {
                    sam_prob_realn(
                        &mut self.inner,
                        rf.as_ptr() as *const c_char,
                        rf.len() as HtsPos,
                        flags,
                    )
                } {
                    0 => Ok(true),
                    -1 => Err(SamError::BaqRealignFailed),
                    -3 => Ok(false), // realignnent not done because already done
                    -4 => Err(SamError::BaqRealignOutOfMem),
                    _ => Err(SamError::BaqRealignUnknownError),
                }
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }
}
