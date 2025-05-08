#![allow(nonstandard_style)]

mod rust_impl;
mod c_impl;
mod parse;

use libc::{c_char, c_int};

const BAM_USER_OWNS_STRUCT: u32 = 1;
#[allow(unused)]
const BAM_USER_OWNS_DATA: u32 = 2;

use crate::{kstring::KString, sam::SamHdrRaw, hts::HtsPos};

pub const BAM_FPAIRED: u16 = 1;
pub const BAM_FPROPER_PAIR: u16 = 2;
pub const BAM_FUNMAP: u16 = 4;
pub const BAM_FMUNMAP: u16 = 8;
pub const BAM_FREVERSE: u16 = 16;
pub const BAM_FMREVERSE: u16 = 32;
pub const BAM_FREAD1: u16 = 64;
pub const BAM_FREAD2: u16 = 128;
pub const BAM_FSECONDARY: u16 = 256;
pub const BAM_FQCFAIL: u16 = 512;
pub const BAM_FDUP: u16 = 1024;
pub const BAM_FSUPPLEMENTARY: u16 = 2048;

pub const SAM_FORMAT_VERSION: &str = "1.6";

#[link(name = "hts")]
unsafe extern "C" {
    fn bam_destroy1(b: *mut bam1_t);
    fn bam_copy1(bdest: *mut bam1_t, bsrc: *const bam1_t) -> *mut bam1_t;
    fn bam_endpos(pt_: *const bam1_t) -> HtsPos;
    fn bam_set_qname(pt_: *mut bam1_t, qname: *const c_char) -> c_int;
    fn sam_parse1(kstring: *mut KString, sam_hdr: *mut SamHdrRaw, b: *mut bam1_t) -> c_int;
}

#[repr(C)]
#[derive(Default)]
struct bam1_core_t {
    pos: HtsPos,
    tid: i32,
    bin: u16,
    qual: u8,
    l_extranul: u8,
    flag: u16,
    l_qname: u16,
    n_cigar: u32,
    l_qseq: i32,
    mtid: i32,
    mpos: HtsPos,
    isze: HtsPos,
}

#[repr(C)]
pub struct bam1_t {
    core: bam1_core_t,
    id: u64,
    data: *mut c_char,
    l_data: c_int,
    m_data: u32,
    mempolicy: u32,
}

impl Default for bam1_t {
    fn default() -> Self {
        Self {
            core: bam1_core_t::default(),
            id: 0,
            data: std::ptr::null_mut(),
            l_data: 0,
            m_data: 0,
            mempolicy: BAM_USER_OWNS_STRUCT,
        }
    }
}
