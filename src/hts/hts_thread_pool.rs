use libc::c_int;
use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

#[repr(C)]
pub struct HtsTPoolRaw {
    _unused: [u8; 0],
}

#[repr(C)]
pub struct HtsTPool<'a> {
    inner: NonNull<HtsTPoolRaw>,
    phantom: PhantomData<&'a mut HtsTPoolRaw>,
}

impl Deref for HtsTPool<'_> {
    type Target = HtsTPoolRaw;

    fn deref(&self) -> &Self::Target {
        unsafe { self.inner.as_ref() }
    }
}

impl DerefMut for HtsTPool<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { self.inner.as_mut() }
    }
}

impl Drop for HtsTPool<'_> {
    fn drop(&mut self) {
        unsafe { hts_tpool_destroy(self.deref_mut()) }
    }
}

unsafe impl Send for HtsTPool<'_> {}
unsafe impl Sync for HtsTPool<'_> {}

#[link(name = "hts")]
unsafe extern "C" {
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

impl HtsTPool<'_> {
    /// Creates a worker pool with n worker threads
    pub fn init(nthreads: usize) -> Option<Self> {
        NonNull::new(unsafe { hts_tpool_init(nthreads as c_int) }).map(|tpool| Self {
            inner: tpool,
            phantom: PhantomData,
        })
    }
}

#[repr(C)]
pub struct HtsThreadPool<'a> {
    inner: HtsTPool<'a>,
    qsize: c_int,
}

impl Deref for HtsThreadPool<'_> {
    type Target = HtsTPoolRaw;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for HtsThreadPool<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // We can do this safely as self.inner is always non-null
        &mut self.inner
    }
}

impl HtsThreadPool<'_> {
    pub fn init(nthreads: usize) -> Option<Self> {
        HtsTPool::init(nthreads).map(|inner| Self { inner, qsize: 0 })
    }

    pub fn as_ptr(&mut self) -> *mut Self {
        self as *mut Self
    }
}
