use std::{iter::FusedIterator, ptr::NonNull};

use c2rust_bitfields::BitfieldStruct;

use libc::{c_int, c_uchar, c_void};

use crate::{
    bgzf::BgzfRaw,
    hts::{
        HtsPos,
        hts_region::HtslibRegion,
        traits::{IdMap, ReadRecIter},
    },
};

#[repr(C)]
struct HtsReglist {
    _unused: [u8; 0],
}

#[repr(C)]
struct HtsPair64Max {
    _unused: [u8; 0],
}

#[repr(C)]
struct HtsItrBins {
    n: c_int,
    m: c_int,
    a: *mut c_int,
}

type HtsReadrecFunc = unsafe extern "C" fn(
    fp: *mut BgzfRaw,
    data: *mut c_void,
    r: *mut c_void,
    tid: *mut c_int,
    beg: *mut HtsPos,
    end: *mut HtsPos,
) -> c_int;

type HtsSeekFunc = unsafe extern "C" fn(fp: *mut c_void, offset: i64, where_: c_int);
type HtsTellFunc = unsafe extern "C" fn(fp: *mut c_void);

#[repr(C)]
#[derive(BitfieldStruct)]
pub(crate) struct HtsItrRaw {
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
    fn hts_itr_destroy(idx: *mut HtsItrRaw);
    pub(crate) fn hts_itr_next(
        fp: *mut BgzfRaw,
        itr: *mut HtsItrRaw,
        r: *mut c_void,
        data: *mut c_void,
    ) -> c_int;
}

pub struct HtsItr {
    inner: NonNull<HtsItrRaw>,
}

impl HtsItr {
    pub(crate) fn deref(&self) -> &HtsItrRaw {
        // We can do this safely as self.inner is always non-null
        unsafe { self.inner.as_ref() }
    }

    pub(crate) fn deref_mut(&mut self) -> &mut HtsItrRaw {
        // We can do this safely as self.inner is always non-null
        unsafe { self.inner.as_mut() }
    }
    
    #[inline]
    pub(crate) fn make(p: *mut HtsItrRaw) -> Option<Self> {
        NonNull::new(p).map(|inner| Self { inner })
    }

    #[inline]
    pub fn finished(&self) -> bool {
        self.deref().finished() != 0
    }
}

unsafe impl Send for HtsItr {}

impl Drop for HtsItr {
    fn drop(&mut self) {
        unsafe { hts_itr_destroy(self.deref_mut()) };
    }
}

pub struct HtsRegionIter<'a, F, I, R> {
    reader: &'a R,
    mk_iter: F,
    reg_iter: I,
    finished: bool,
}

impl<'a, F, I, R> HtsRegionIter<'a, F, I, R>
where
    F: Fn(&HtslibRegion) -> HtsItr,
    I: Iterator<Item = HtslibRegion> + FusedIterator,
    R: ReadRecIter + IdMap,
{
    pub fn make(reader: &'a R, reg_iter: I, mk_iter: F) -> Self {
        Self {
            reader,
            mk_iter,
            reg_iter,
            finished: false,
        }
    }
}

impl<'a, F, I, R> Iterator for HtsRegionIter<'a, F, I, R>
where
    F: Fn(&HtslibRegion) -> HtsItr,
    I: Iterator<Item = HtslibRegion> + FusedIterator,
    R: ReadRecIter + IdMap,
{
    type Item = HtsItr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            None
        } else {
            self.reg_iter
                .next()
                .map(|r| (self.mk_iter)(&r))
                .or_else(|| {
                    self.finished = true;
                    None
                })
        }
    }
}
