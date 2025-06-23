use std::{iter::FusedIterator, ptr::NonNull};

use c2rust_bitfields::BitfieldStruct;

use libc::{c_int, c_uchar, c_void};

use crate::{
    bgzf::BgzfRaw,
    hts::{
        hts_region::HtslibRegion, traits::{HdrType, IdMap, ReadRec, ReadRecIter}, HtsPos
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

pub(crate) struct HtsRegionSubIter<F, I> 
where
    F: Fn(&HtslibRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtslibRegion>,
{
    mk_iter: F,
    reg_iter: I,
    finished: bool,
}

impl<F, I> HtsRegionSubIter<F, I>
where
    F: Fn(&HtslibRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtslibRegion>,
{
    pub(crate) fn make(reg_iter: I, mk_iter: F) -> Self {
        Self {
            mk_iter,
            reg_iter,
            finished: false,
        }
    }
}

impl<F, I> Iterator for HtsRegionSubIter<F, I>
where
    F: Fn(&HtslibRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtslibRegion>,
{
    type Item = HtsItr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            None
        } else {
            self.reg_iter
                .next()
                .and_then(|r| (self.mk_iter)(&r))
                .or_else(|| {
                    self.finished = true;
                    None
                })
        }
    }
}

impl<F, I> FusedIterator for HtsRegionSubIter<F, I>
where
    F: Fn(&HtslibRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtslibRegion>,
{
}

pub struct HtsRegionIter<F, R, I> 
where
    F: Fn(&HtslibRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtslibRegion>,
    R: ReadRecIter,
{
    sub_iter: Option<HtsRegionSubIter<F, I>>,
    read_rec: R,
    current_iter: Option<HtsItr>,
}

impl<F, R, I> HtsRegionIter<F, R, I>
where
    F: Fn(&HtslibRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtslibRegion>,
    R: ReadRecIter,
{
    pub(crate) fn make(mut iter: HtsRegionSubIter<F, I>, read_rec: R) -> Self {
        
        // let mut iter = HtsRegionSubIter::make(reg_iter, |r| (*self.mk_iter)());
        let (sub_iter, current_iter) = if let Some(itr) = iter.next() {
            (Some(iter), Some(itr))
        } else {
            (None, None)
        };

        Self {
            sub_iter,
            read_rec,
            current_iter,
        }
    }
}

impl<F, R, I> ReadRec for HtsRegionIter<F, R, I>
where
    F: Fn(&HtslibRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtslibRegion>,
    R: ReadRecIter,
{
    type Err = R::Err;
    type Rec = R::Rec;

    fn read_rec(&mut self, rec: &mut Self::Rec) -> Result<Option<()>, Self::Err> {
        loop {
            match (self.current_iter.as_mut(), self.sub_iter.as_mut()) {
                // Iterator finished
                (None, None) => break Ok(None),
                (Some(itr), _) => {
                    if self.read_rec.read_rec_iter(itr, rec)?.is_some() {
                        break Ok(Some(()));
                    } else {
                        self.current_iter = None;
                    }
                }
                (None, Some(iter)) => {
                    self.current_iter = iter.next();
                    if self.current_iter.is_none() {
                        self.sub_iter = None;
                    }
                }
            }
        }
    }
}

impl<F, R, I> HdrType for HtsRegionIter<F, R, I>
where
    F: Fn(&HtslibRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtslibRegion>,
    R: ReadRecIter + HdrType,
{
    fn hdr_type(&self) -> super::traits::HtsHdrType {
        self.read_rec.hdr_type()
    }
}

impl<F, R, I> IdMap for HtsRegionIter<F, R, I>
where
    F: Fn(&HtslibRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtslibRegion>,
    R: ReadRecIter + IdMap,
{
    fn seq_name(&self, i: usize) -> Option<&std::ffi::CStr> {
        self.read_rec.seq_name(i)
    }
    
    fn num_seqs(&self) -> usize {
        self.read_rec.num_seqs()
    }
    
    fn seq_len(&self, i: usize) -> Option<usize> {
        self.read_rec.seq_len(i)
    }
}