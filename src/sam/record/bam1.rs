#![allow(nonstandard_style)]

use std::{ffi::CStr, ptr::copy_nonoverlapping};
mod aux;
pub mod aux_error;
mod parse;
mod record_impl;
mod rust_impl;
mod seq_iter;

use libc::{c_char, c_int, c_void, realloc};

const BAM_USER_OWNS_STRUCT: u32 = 1;
#[allow(unused)]
const BAM_USER_OWNS_DATA: u32 = 2;

use crate::{hts::HtsPos, SamError};

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
    fn bam_copy1<'a>(bdest: *mut bam1_t, bsrc: *const bam1_t) -> *mut bam1_t;
    fn bam_endpos(pt_: *const bam1_t) -> HtsPos;
    fn bam_set_qname(pt_: *mut bam1_t, qname: *const c_char) -> c_int;
}

#[repr(C)]
#[derive(Default, Debug)]
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
#[derive(Debug)]
pub(super) struct bam1_t {
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

impl bam1_t {
    /// In common with standard rust memory allocation, we panic if memory is not available
    /// or if allocation requested is too large
    fn realloc_data(&mut self, size: usize) {
        // Can only use this with htslib managed data
        assert_eq!(self.mempolicy & BAM_USER_OWNS_DATA, 0);
        let s = crate::roundup(size);
        assert!(
            s <= c_int::MAX as usize,
            "Requested allocation size is too large for Bam Record"
        );
        let new_data = unsafe { realloc(self.data as *mut c_void, s) };
        assert!(!new_data.is_null(), "Out of memory");

        self.data = new_data as *mut c_char;
        self.m_data = s as u32;
        self.l_data = self.l_data.min(s as c_int);
    }

    #[inline]
    fn reserve(&mut self, additional: usize) {
        let sz = (self.l_data as usize)
            .checked_add(additional)
            .expect("Allocation size too high");
        if sz > self.m_data as usize {
            self.realloc_data(sz)
        }
    }

    #[inline]
    fn copy_data<T: Sized>(&mut self, src: &[T]) {
        let sz = size_of_val(src);
        self.reserve(sz);

        unsafe {
            copy_nonoverlapping(
                src.as_ptr(),
                self.data.add(self.l_data as usize) as *mut T,
                src.len(),
            );
        }
        self.l_data += sz as i32;
    }

    #[inline]
    fn push_char(&mut self, b: u8) {
        self.reserve(1);
        unsafe { *self.data.add(self.l_data as usize) = b as c_char }
        self.l_data += 1
    }

    fn copy(&self, dst: &mut Self) {
        if unsafe { bam_copy1(dst, self) }.is_null() {
            panic!("Out of memory copying Bam record")
        }
    }

    fn end_pos(&self) -> HtsPos {
        unsafe { bam_endpos(self) }
    }
    
    fn set_query_name(&mut self, qname: &CStr) -> Result<(), SamError> {
        match unsafe { bam_set_qname(self, qname.as_ptr()) } {
            0 => Ok(()),
            _ => Err(SamError::SetQnameFailed),
        }
    }
}
