use crate::hts::htsfile::HtsFile;
use libc::{c_char, c_int, malloc_usable_size};
use std::ops::{Deref, DerefMut};

#[repr(C)]
pub struct HtsTPoolRaw {
    _unused: [u8; 0],
}

#[repr(C)]
pub struct HtsTPool {
    inner: *mut HtsTPoolRaw,
}

impl Deref for HtsTPool {
    type Target = HtsTPoolRaw;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner }
    }
}

impl DerefMut for HtsTPool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { &mut *self.inner }
    }
}

impl Drop for HtsTPool {
    fn drop(&mut self) {
        unsafe { hts_tpool_destroy(self.inner) }
    }
}

unsafe impl Send for HtsTPool {}
unsafe impl Sync for HtsTPool {}

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

impl HtsTPool {
    /// Creates a worker pool with n worker threads
    pub fn init(nthreads: usize) -> Option<Self> {
        let tpool = unsafe { hts_tpool_init(nthreads as c_int) };
        if tpool.is_null() {
            None
        } else {
            Some(Self { inner: tpool })
        }
    }
}

#[repr(C)]
pub struct HtsThreadPool {
    inner: HtsTPool,
    qsize: c_int,
}

impl Deref for HtsThreadPool {
    type Target = HtsTPoolRaw;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner }
    }
}

impl DerefMut for HtsThreadPool {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { &mut *self.inner }
    }
}

impl HtsThreadPool {
    pub fn init(nthreads: usize) -> Option<Self> {
        HtsTPool::init(nthreads).map(|inner| Self { inner, qsize: 0 })
    }
}
