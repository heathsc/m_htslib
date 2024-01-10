use libc::c_int;
use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

#[repr(C)]
pub struct HtsTPoolRaw {
    _unused: [u8; 0],
}

#[repr(C)]
pub struct HtsTPool<'a> {
    inner: *mut HtsTPoolRaw,
    phantom: PhantomData<&'a HtsTPoolRaw>,
}

impl<'a> Deref for HtsTPool<'a> {
    type Target = HtsTPoolRaw;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner }
    }
}

impl<'a> DerefMut for HtsTPool<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { &mut *self.inner }
    }
}

impl<'a> Drop for HtsTPool<'a> {
    fn drop(&mut self) {
        unsafe { hts_tpool_destroy(self.inner) }
    }
}

unsafe impl<'a> Send for HtsTPool<'a> {}
unsafe impl<'a> Sync for HtsTPool<'a> {}

#[link(name = "hts")]
extern "C" {
    fn hts_tpool_init(n: c_int) -> *mut HtsTPoolRaw;
    fn hts_tpool_destroy(p: *mut HtsTPoolRaw);
    fn hts_tpool_size(p: *const HtsTPoolRaw) -> c_int;
}

impl HtsTPoolRaw {
    /// Returns the number of requested threads for a pool.
    pub fn size(&self) -> usize {
        unsafe { hts_tpool_size(self) as usize }
    }
}

impl<'a> HtsTPool<'a> {
    /// Creates a worker pool with n worker threads
    pub fn init(nthreads: usize) -> Option<Self> {
        let tpool = unsafe { hts_tpool_init(nthreads as c_int) };
        if tpool.is_null() {
            None
        } else {
            Some(Self {
                inner: tpool,
                phantom: PhantomData,
            })
        }
    }
}

#[repr(C)]
pub struct HtsThreadPool<'a> {
    inner: HtsTPool<'a>,
    qsize: c_int,
}

impl<'a> Deref for HtsThreadPool<'a> {
    type Target = HtsTPoolRaw;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner }
    }
}

impl<'a> DerefMut for HtsThreadPool<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { &mut *self.inner }
    }
}

impl<'a> HtsThreadPool<'a> {
    pub fn init(nthreads: usize) -> Option<Self> {
        HtsTPool::init(nthreads).map(|inner| Self { inner, qsize: 0 })
    }

    pub fn as_ptr(&mut self) -> *mut Self {
        self as *mut Self
    }
}
