use std::{
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use c2rust_bitfields::BitfieldStruct;

use libc::{c_char, c_int, c_short, c_uchar, c_uint, c_void, off_t, size_t, ssize_t};

use crate::{bgzf::BgzfRaw, hts::HtsPos};

#[repr(C)]
pub struct HtsReglist {
    _unused: [u8; 0],
}

#[repr(C)]
pub struct HtsPair64Max {
    _unused: [u8; 0],
}

#[repr(C)]
pub struct HtsItrBins {
    n: c_int,
    m: c_int,
    a: *mut c_int,
}

pub type HtsReadrecFunc = unsafe extern "C" fn(
    fp: *mut BgzfRaw,
    data: *mut c_void,
    r: *mut c_void,
    tid: *mut c_int,
    beg: *mut HtsPos,
    end: *mut HtsPos,
) -> c_int;

pub type HtsSeekFunc = unsafe extern "C" fn(fp: *mut c_void, offset: i64, where_: c_int);
pub type HtsTellFunc = unsafe extern "C" fn(fp: *mut c_void);

#[repr(C)]
#[derive(BitfieldStruct)]
pub struct HtsItrRaw {
    #[bitfield(name = "read_rest", ty = "c_uchar", bits = "0..=0")]
    #[bitfield(name = "finished", ty = "c_uchar", bits = "1..=1")]
    #[bitfield(name = "is_cram", ty = "c_uchar", bits = "2..=2")]
    #[bitfield(name = "nocoor", ty = "c_uchar", bits = "3..=3")]
    #[bitfield(name = "multi", ty = "c_uchar", bits = "4..=4")]
    #[bitfield(name = "dummy", ty = "u32", bits = "5..=31")]
    bfield: [u8; 4],
    tid: c_int,
    n_off: c_int,
    i: c_int,
    n_reg: c_int,
    beg: HtsPos,
    end: HtsPos,
    reg_list: *mut HtsReglist,
    curr_tid: c_int,
    curr_reg: c_int,
    curr_intv: c_int,
    curr_beg: HtsPos,
    curr_end: HtsPos,
    curr_off: u64,
    nocoor_off: u64,
    off: *mut HtsPair64Max,
    readrec: HtsReadrecFunc,
    seek: HtsSeekFunc,
    tell: HtsTellFunc,
    bins: HtsItrBins,
}

#[link(name = "hts")]
unsafe extern "C" {
       pub(super) fn hts_itr_destroy(idx: *mut HtsItrRaw);
    
}

pub struct HtsItr {
    inner: NonNull<HtsItrRaw>
}

impl Deref for HtsItr {
    type Target = HtsItrRaw;

    fn deref(&self) -> &Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { self.inner.as_ref() }
    }
}

impl DerefMut for HtsItr {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { self.inner.as_mut() }
    }
}

unsafe impl Send for HtsItr {}

impl Drop for HtsItr {
    fn drop(&mut self) {
        unsafe { hts_itr_destroy(self.deref_mut()) };
    }
}


