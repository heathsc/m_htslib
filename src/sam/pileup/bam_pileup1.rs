use std::ptr::NonNull;

use c2rust_bitfields::BitfieldStruct;
use libc::{c_int, c_void};

use super::BamPileup;
use crate::sam::{BamRec, bam1_t};

#[repr(C)]
pub(super) struct bam_plp_t {
    _unused: [u8; 0],
}

#[repr(C)]
pub(super) struct bam_mplp_t {
    _unused: [u8; 0],
}

#[repr(C)]
pub(super) union bam_pileup_cd {
    p: *mut c_void,
    i: i64,
    f: f64,
}

#[repr(C)]
#[derive(BitfieldStruct)]
pub(super) struct bam_pileup1_t {
    pub(super) b: *mut bam1_t,
    pub(super) qpos: i32,
    pub(super) indel: c_int,
    level: c_int,
    #[bitfield(name = "is_del", ty = "bool", bits = "0..=0")]
    #[bitfield(name = "is_head", ty = "bool", bits = "1..=1")]
    #[bitfield(name = "is_tail", ty = "bool", bits = "2..=2")]
    #[bitfield(name = "is_refskip", ty = "bool", bits = "3..=3")]
    #[bitfield(name = "aux", ty = "u32", bits = "4..=31")]
    bfield: [u8; 4],
    _cd: bam_pileup_cd, // Not currently used
    cigar_ind: c_int,
}

pub(super) type BamPlpAuto = extern "C" fn(data: *mut c_void, b: *mut bam1_t) -> c_int;

impl BamPileup<'_> {
    pub fn bam_rec(&self) -> &BamRec {
        let b = NonNull::new(self.as_ref().b as *mut BamRec).expect("Bam record null in pileup");
        unsafe { b.as_ref() }
    }

    #[inline]
    pub fn is_del(&self) -> bool {
        self.as_ref().is_del()
    }

    #[inline]
    pub fn is_head(&self) -> bool {
        self.as_ref().is_head()
    }
    
    #[inline]
    pub fn is_tail(&self) -> bool {
        self.as_ref().is_tail()
    }
    
    #[inline]
    pub fn is_refskip(&self) -> bool {
        self.as_ref().is_refskip()
    }
    
    #[inline]
    pub fn qpos(&self) -> i32 {
        self.as_ref().qpos
    }

    #[inline]
    pub fn indel(&self) -> c_int {
        self.as_ref().indel
    }

    #[inline]
    pub fn level(&self) -> c_int {
        self.as_ref().level
    }

    #[inline]
    pub fn cigar_ind(&self) -> c_int {
        self.as_ref().cigar_ind
    }
}
