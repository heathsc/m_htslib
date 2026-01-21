use std::{
    iter::FusedIterator,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use c2rust_bitfields::BitfieldStruct;

use libc::{c_int, c_uchar, c_void};

use crate::{
    bgzf::BgzfRaw,
    hts::{
        HtsPos,
        hts_region::HtsRegion,
        traits::{HdrType, IdMap, ReadRec, ReadRecIter},
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

struct HtsRegionSubIter<F, I>
where
    F: Fn(&HtsRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtsRegion>,
{
    mk_hts_iter: F,
    reg_iter: I,
    finished: bool,
}

impl<F, I> HtsRegionSubIter<F, I>
where
    F: Fn(&HtsRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtsRegion>,
{
    fn make(reg_iter: I, mk_iter: F) -> Self {
        Self {
            mk_hts_iter: mk_iter,
            reg_iter,
            finished: false,
        }
    }
}

impl<F, I> Iterator for HtsRegionSubIter<F, I>
where
    F: Fn(&HtsRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtsRegion>,
{
    type Item = HtsItr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            None
        } else {
            self.reg_iter
                .next()
                .and_then(|r| (self.mk_hts_iter)(&r))
                .or_else(|| {
                    self.finished = true;
                    None
                })
        }
    }
}

impl<F, I> FusedIterator for HtsRegionSubIter<F, I>
where
    F: Fn(&HtsRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtsRegion>,
{
}

pub struct HtsRegionsIter<F, R, I>
where
    F: Fn(&HtsRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtsRegion>,
{
    sub_iter: Option<HtsRegionSubIter<F, I>>,
    iter: HtsRegionIter<R>,
}

pub struct HtsRegionIter<R> {
    read_rec: R,
    current_iter: Option<HtsItr>,
}

impl<F, R, I> Deref for HtsRegionsIter<F, R, I>
where
    F: Fn(&HtsRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtsRegion>,
{
    type Target = HtsRegionIter<R>;

    fn deref(&self) -> &Self::Target {
        &self.iter
    }
}

impl<F, R, I> DerefMut for HtsRegionsIter<F, R, I>
where
    F: Fn(&HtsRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtsRegion>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.iter
    }
}

impl<F, R, I> HtsRegionsIter<F, R, I>
where
    F: Fn(&HtsRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtsRegion>,
    R: ReadRecIter,
{
    pub fn make_regions_iter(reg_iter: I, mk_hts_iter: F, read_rec: R) -> Self {
        let mut iter = HtsRegionSubIter::make(reg_iter, mk_hts_iter);
        let (sub_iter, current_iter) = if let Some(itr) = iter.next() {
            (Some(iter), Some(itr))
        } else {
            (None, None)
        };

        Self {
            sub_iter,
            iter: HtsRegionIter {
                read_rec,
                current_iter,
            },
        }
    }
}

impl<R> HtsRegionIter<R>
where
    R: ReadRecIter,
{
    pub fn make_region_iter<F: Fn(&HtsRegion) -> Option<HtsItr>>(
        region: HtsRegion,
        mk_hts_iter: F,
        read_rec: R,
    ) -> Self {
        Self {
            read_rec,
            current_iter: mk_hts_iter(&region),
        }
    }
}

impl<F, R, I> ReadRec for HtsRegionsIter<F, R, I>
where
    F: Fn(&HtsRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtsRegion>,
    R: ReadRecIter,
{
    type Err = R::Err;
    type Rec = R::Rec;

    fn read_rec(&mut self, rec: &mut Self::Rec) -> Result<Option<()>, Self::Err> {
        loop {
            match (self.iter.current_iter.as_mut(), self.sub_iter.as_mut()) {
                // Iterator finished
                (None, None) => break Ok(None),
                (Some(itr), _) => {
                    if self.iter.read_rec.read_rec_iter(itr, rec)?.is_some() {
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

impl<R> ReadRec for HtsRegionIter<R>
where
    R: ReadRecIter,
{
    type Err = R::Err;
    type Rec = R::Rec;

    fn read_rec(&mut self, rec: &mut Self::Rec) -> Result<Option<()>, Self::Err> {
        match self.current_iter.as_mut() {
            Some(itr) => self.read_rec.read_rec_iter(itr, rec),
            None => Ok(None),
        }
    }
}

impl<R> HdrType for HtsRegionIter<R>
where
    R: ReadRecIter + HdrType,
{
    fn hdr_type(&self) -> super::traits::HtsHdrType {
        self.read_rec.hdr_type()
    }
}

impl<R> IdMap for HtsRegionIter<R>
where
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

impl<F, R, I> HdrType for HtsRegionsIter<F, R, I>
where
    F: Fn(&HtsRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtsRegion>,
    R: ReadRecIter + HdrType,
{
    fn hdr_type(&self) -> super::traits::HtsHdrType {
        self.read_rec.hdr_type()
    }
}

impl<F, R, I> IdMap for HtsRegionsIter<F, R, I>
where
    F: Fn(&HtsRegion) -> Option<HtsItr>,
    I: Iterator<Item = HtsRegion>,
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
