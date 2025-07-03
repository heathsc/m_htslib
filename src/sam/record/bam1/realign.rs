use libc::{c_char, c_int};

use crate::{hts::HtsPos, sam::BamRec};

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
    fn sam_prob_realn(b: *mut bam1_t, rf: *const c_char,rf_len: HtsPos, flag: c_int) -> c_int;
}

