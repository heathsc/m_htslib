use std::{marker::PhantomData, ptr::NonNull};

use libc::{c_int, c_void};

use super::{
    bam_pileup1::{BamPlpAuto, bam_mplp_t, bam_pileup1_t, bam_plp_t},
    c_func_calls,
};
use crate::{
    SamError,
    hts::{HtsPos, ReadRec},
    sam::BamRec,
};

pub struct BamPileup {
    inner: bam_pileup1_t
}

impl BamPileup {
    pub(super) fn as_ref(&self) -> &bam_pileup1_t {
        &self.inner
    }
}

impl Drop for BamPlp {
    fn drop(&mut self) {
        unsafe { c_func_calls::bam_plp_destroy(self.inner.as_mut()) };
    }
}

pub struct BamPlp {
    inner: NonNull<bam_plp_t>,
}

impl BamPlp {
    pub fn init<R: ReadRec<Rec = BamRec>>(data: &mut R) -> Self {
        let f: extern "C" fn(*mut R, *mut R::Rec) -> c_int = bam_plp_callback::<R>;
        let ptr: BamPlpAuto = unsafe { std::mem::transmute(f) };
        let inner = NonNull::new(unsafe {
            super::c_func_calls::bam_plp_init(ptr, data as *mut R as *mut c_void)
        })
        .expect("bam_mplp_init failed");
        Self { inner }
    }

    pub fn push(&mut self, b: &BamRec) {
        unsafe { c_func_calls::bam_plp_push(self.inner.as_mut() as *mut bam_plp_t, b.as_ptr()) }
    }

    pub fn next<'a>(&'a mut self) -> Result<Option<(&'a [BamPileup], c_int, HtsPos)>, SamError> {
        let mut pos: HtsPos = 0;
        let mut tid: c_int = -1;
        let mut n: c_int = 0;
        let plp = unsafe {
            c_func_calls::bam_plp64_next(
                self.inner.as_mut() as *mut bam_plp_t,
                &mut tid as *mut c_int,
                &mut pos as *mut HtsPos,
                &mut n as *mut c_int,
            )
        };
        make_plp_return(plp, tid, pos, n)
    }

    pub fn auto<'a>(&'a mut self) -> Result<Option<(&'a [BamPileup], c_int, HtsPos)>, SamError> {
        let mut pos: HtsPos = 0;
        let mut tid: c_int = -1;
        let mut n: c_int = 0;
        let plp = unsafe {
            c_func_calls::bam_plp64_auto(
                self.inner.as_mut() as *mut bam_plp_t,
                &mut tid as *mut c_int,
                &mut pos as *mut HtsPos,
                &mut n as *mut c_int,
            )
        };
        make_plp_return(plp, tid, pos, n)
    }

    pub fn set_maxcnt(&mut self, max_cnt: c_int) {
        assert!(max_cnt > 0, "Invalid maxcnt value {max_cnt}");
        unsafe { c_func_calls::bam_plp_set_maxcnt(self.inner.as_mut() as *mut bam_plp_t, max_cnt) };
    }

    pub fn reset(&mut self) {
        unsafe { c_func_calls::bam_plp_reset(self.inner.as_mut() as *mut bam_plp_t) };
    }
}

fn make_plp_return<'a>(
    plp: *const bam_pileup1_t,
    tid: c_int,
    pos: HtsPos,
    n: c_int,
) -> Result<Option<(&'a [BamPileup], c_int, HtsPos)>, SamError> {
    if n < 0 {
        Err(SamError::PileupError)
    } else if n == 0 || plp.is_null() {
        Ok(None)
    } else {
        let plp = unsafe { std::slice::from_raw_parts(plp as *const BamPileup, n as usize) };
        Ok(Some((plp, tid, pos)))
    }
}

pub struct BamMPlp<'a, T> {
    inner: NonNull<bam_mplp_t>,
    plp_raw: Box<[*const bam_pileup1_t]>,
    depth: Box<[c_int]>,
    _data: BamMPlpData<'a, T>,
}

impl<T> Drop for BamMPlp<'_, T> {
    fn drop(&mut self) {
        unsafe { c_func_calls::bam_mplp_destroy(self.inner.as_mut()) };
    }
}

impl<'a, R> BamMPlp<'a, R>
where
    R: ReadRec<Rec = BamRec>,
{
    pub fn init(data: &'a mut [R]) -> Self {
        let v: Box<_> = data.iter_mut().map(|p| p as *mut R).collect();
        let n = v.len();

        let mut d = BamMPlpData {
            inner: v,
            _phantom: PhantomData,
        };

        let plp_raw: Box<[*const bam_pileup1_t]> = vec![std::ptr::null(); n].into_boxed_slice();

        let depth: Box<[c_int]> = vec![0; n].into_boxed_slice();

        let f: extern "C" fn(*mut R, *mut R::Rec) -> c_int = bam_plp_callback::<R>;
        let ptr: BamPlpAuto = unsafe { std::mem::transmute(f) };
        let inner = NonNull::new(unsafe {
            super::c_func_calls::bam_mplp_init(
                d.inner.len() as c_int,
                ptr,
                d.inner.as_mut_ptr() as *mut *mut c_void,
            )
        })
        .expect("bam_mplp_init failed");

        Self {
            inner,
            plp_raw,
            depth,
            _data: d,
        }
    }

    pub fn init_overlaps(&mut self) {
        unsafe { c_func_calls::bam_mplp_init_overlaps(self.inner.as_mut() as *mut bam_mplp_t) };
    }

    pub fn auto(&mut self) -> Result<Option<(c_int, HtsPos)>, SamError> {
        // Make sure previous results cannot be used
        self.plp_raw.fill(std::ptr::null());
        self.depth.fill(0);

        let mut pos: HtsPos = 0;
        let mut tid: c_int = -1;

        let plp_raw = self.plp_raw.as_mut_ptr();
        let ret = unsafe {
            c_func_calls::bam_mplp64_auto(
                self.inner.as_mut(),
                &mut tid as *mut c_int,
                &mut pos as *mut HtsPos,
                self.depth.as_mut_ptr(),
                plp_raw,
            )
        };
        if ret < 0 {
            Err(SamError::PileupError)
        } else if ret == 0 {
            Ok(None)
        } else {
            Ok(Some((tid, pos)))
        }
    }

    pub fn set_maxcnt(&mut self, max_cnt: c_int) {
        assert!(max_cnt > 0, "Invalid max_cnt value {max_cnt}");
        unsafe {
            c_func_calls::bam_mplp_set_maxcnt(self.inner.as_mut() as *mut bam_mplp_t, max_cnt)
        };
    }

    pub fn reset(&mut self) {
        unsafe { c_func_calls::bam_mplp_reset(self.inner.as_mut() as *mut bam_mplp_t) };
    }

    pub fn plp(&self, ix: usize) -> Option<&[BamPileup]> {
        self.plp_raw.get(ix).and_then(|q| {
            if q.is_null() {
                None
            } else {
                let n = self.depth[ix] as usize;
                Some (
                    unsafe {
                        std::slice::from_raw_parts(*q as *const BamPileup, n)
                    }
                )
            }
        })
    }
}

pub struct BamMPlpData<'a, T> {
    inner: Box<[*mut T]>,
    _phantom: PhantomData<&'a mut T>,
}

extern "C" fn bam_plp_callback<R: ReadRec<Rec = BamRec>>(d: *mut R, b: *mut R::Rec) -> c_int {
    let d = unsafe {
        NonNull::new(d as *mut R)
            .expect("Zero pointer for data recieved")
            .as_mut()
    };
    let b = unsafe {
        NonNull::new(b)
            .expect("Zero pointer for record recieved")
            .as_mut()
    };
    match d.read_rec(b) {
        Ok(Some(_)) => 0,
        Ok(None) => -1,
        Err(_) => -2,
    }
}
