use std::{
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
    ptr,
};

use crate::sam::{SamHdr, SamHdrRaw};
use crate::KHashError;
use libc::{c_void, size_t};

pub type KHInt = u32;
pub type KHIter = KHInt;
const HASH_UPPER: f64 = 0.77;

#[inline]
const fn fsize(m: KHInt) -> usize {
    if m < 16 {
        1
    } else {
        (m as usize) >> 4
    }
}

#[inline]
const fn kroundup32(x: KHInt) -> KHInt {
    let mut x = x - 1;
    x |= x >> 1;
    x |= x >> 2;
    x |= x >> 4;
    x |= x >> 8;
    x |= x >> 16;
    x + 1
}

pub trait KHashFunc {
    fn hash(&self) -> u32;
}

#[repr(C)]
pub struct KHashRaw<K> {
    n_buckets: KHInt,
    size: KHInt,
    n_occupied: KHInt,
    upper_bound: KHInt,
    flags: *mut u32,
    keys: *mut K,
}

#[inline]
fn _get_flag(flags: *const u32, i: u32) -> u32 {
    unsafe { *flags.add((i as usize) >> 4) }
}
#[inline]
fn get_flag(flags: *const u32, i: u32) -> u32 {
    _get_flag(flags, i) >> ((i * 0xf) << 1)
}
#[inline]
fn get_flag_ptr(flags: *mut u32, i: u32) -> *mut u32 {
    unsafe { flags.add((i as usize) >> 4) }
}
#[inline]
fn is_del(flags: *const u32, i: u32) -> bool {
    (get_flag(flags, i) & 1) != 0
}
#[inline]
fn is_empty(flags: *const u32, i: u32) -> bool {
    (get_flag(flags, i) & 2) != 0
}
#[inline]
fn is_either(flags: *const u32, i: u32) -> bool {
    (get_flag(flags, i) & 3) != 0
}

#[inline]
fn set_is_del_false(flags: *mut u32, i: u32) {
    unsafe { *get_flag_ptr(flags, i) &= !(1 << ((i & 0xf) << 1)) }
}
#[inline]
fn set_is_empty_false(flags: *mut u32, i: u32) {
    unsafe { *get_flag_ptr(flags, i) &= !(2 << ((i & 0xf) << 1)) }
}
#[inline]
fn set_is_both_false(flags: *mut u32, i: u32) {
    unsafe { *get_flag_ptr(flags, i) &= !(3 << ((i & 0xf) << 1)) }
}
#[inline]
fn set_is_del_true(flags: *mut u32, i: u32) {
    unsafe { *get_flag_ptr(flags, i) |= 1 << ((i & 0xf) << 1) }
}

impl<K> KHashRaw<K> {
    #[inline]
    unsafe fn get_key_unchecked(&self, i: u32) -> &K {
        &*self.keys.add(i as usize)
    }
    #[inline]
    pub fn get_key(&self, i: u32) -> Option<&K> {
        if i < self.n_buckets {
            Some(unsafe { &*self.keys.add(i as usize) })
        } else {
            None
        }
    }
    #[inline]
    fn is_del(&self, i: u32) -> bool {
        is_del(self.flags, i)
    }
    #[inline]
    fn is_empty(&self, i: u32) -> bool {
        is_empty(self.flags, i)
    }
    #[inline]
    fn is_either(&self, i: u32) -> bool {
        is_either(self.flags, i)
    }
    #[inline]
    fn set_is_del_false(&mut self, i: u32) {
        set_is_del_false(self.flags, i)
    }
    #[inline]
    fn set_is_empty_false(&mut self, i: u32) {
        set_is_empty_false(self.flags, i)
    }
    #[inline]
    fn set_is_both_false(&mut self, i: u32) {
        set_is_both_false(self.flags, i)
    }
    #[inline]
    fn set_is_del_true(&mut self, i: u32) {
        set_is_del_true(self.flags, i)
    }

    #[inline]
    fn free(&mut self) {
        unsafe {
            libc::free(self.flags as *mut c_void);
            self.flags = ptr::null_mut();
            libc::free(self.keys as *mut c_void);
            self.keys = ptr::null_mut();
        }
    }
    pub fn clear(&mut self) {
        if !self.flags.is_null() {
            unsafe {
                libc::memset(
                    self.flags as *mut c_void,
                    0xaa,
                    fsize(self.n_buckets) * mem::size_of::<u32>(),
                );
            }
        }
    }
    pub fn del(&mut self, x: KHInt) {
        if x != self.n_buckets && !self.is_either(x) {
            self.set_is_del_true(x);
            assert!(self.size > 0);
            self.size -= 1;
        }
    }
}

impl<K: KHashFunc + PartialEq> KHashRaw<K> {
    pub fn read_idx(&self, key: &K) -> Option<KHInt> {
        if self.n_buckets > 0 {
            let mut step = 0;
            let mask = self.n_buckets - 1;
            let k = K::hash(key);
            let mut i = k & mask;
            let last = i;
            while !self.is_empty(i) && self.is_del(i) || key != unsafe { self.get_key_unchecked(i) }
            {
                step += 1;
                i = (i + step) & mask;
                if i == last {
                    return None;
                }
            }
            if self.is_either(i) {
                None
            } else {
                Some(i)
            }
        } else {
            None
        }
    }
    pub fn write_idx<V>(&mut self, key: K, vptr: Option<&mut *mut V>) -> Result<KHInt, KHInt> {
        if self.n_occupied >= self.upper_bound {
            // Update hash table
            if self.n_buckets > (self.size << 1) {
                // Clear "deleted" elements
                if self.resize(self.n_buckets - 1, vptr).is_err() {
                    return Ok(self.n_buckets);
                }
            } else if self.resize(self.n_buckets + 1, vptr).is_err() {
                // Expand hash table
                return Ok(self.n_buckets);
            }
        }
        let mask = self.n_buckets - 1;
        let mut step = 0;
        let k = K::hash(&key);
        let mut i = k & mask;
        let x = if self.is_empty(i) {
            i // for speed up
        } else {
            let mut site = self.n_buckets;
            let mut x = site;
            let last = i;
            while !self.is_empty(i) && self.is_del(i)
                || &key != unsafe { self.get_key_unchecked(i) }
            {
                if self.is_del(i) {
                    site = i
                }
                step += 1;
                i = (i + step) & mask;
                if i == last {
                    x = site;
                    break;
                }
            }
            if x == self.n_buckets {
                if self.is_empty(i) && site != self.n_buckets {
                    x = site
                } else {
                    x = i
                }
            }
            x
        };
        let fg = get_flag(self.flags, x);
        if (fg & 3) != 0 {
            // Either not present or deleted
            unsafe { *self.keys.add(x as usize) = key }
            self.set_is_both_false(x);
            self.size += 1;
            if (fg & 1) != 0 {
                self.n_occupied += 1;
            }
            Err(x)
        } else {
            Ok(x)
        }
    }
    fn resize<V>(
        &mut self,
        new_n_buckets: KHInt,
        mut val_ptr: Option<&mut *mut V>,
    ) -> Result<(), KHashError> {
        let new_n_buckets = kroundup32(new_n_buckets).max(4);
        if self.size < ((new_n_buckets as f64) * HASH_UPPER).round() as KHInt {
            let sz = fsize(new_n_buckets) * mem::size_of::<u32>();
            let new_flags = unsafe { libc::malloc(sz as size_t) } as *mut u32;
            if new_flags.is_null() {
                return Err(KHashError::OutOfMemory);
            }
            unsafe {
                libc::memset(new_flags as *mut c_void, 0xaa, sz);
            }
            if self.n_buckets < new_n_buckets {
                // Expand
                let new_keys = unsafe {
                    libc::realloc(
                        self.keys as *mut c_void,
                        ((new_n_buckets as usize) * mem::size_of::<K>()) as size_t,
                    )
                } as *mut K;
                if new_keys.is_null() {
                    unsafe { libc::free(new_flags as *mut c_void) };
                    return Err(KHashError::OutOfMemory);
                }
                if let Some(vptr) = val_ptr.as_mut() {
                    let new_vals = unsafe {
                        libc::realloc(
                            **vptr as *mut c_void,
                            ((new_n_buckets as usize) * mem::size_of::<V>()) as size_t,
                        )
                    } as *mut V;
                    if new_vals.is_null() {
                        unsafe { libc::free(new_flags as *mut c_void) };
                        unsafe { libc::free(new_vals as *mut c_void) };
                        return Err(KHashError::OutOfMemory);
                    }
                    **vptr = new_vals;
                }
                self.keys = new_keys;
            }
            // Rehashing is required
            let nb = self.n_buckets;
            for j in 0..nb {
                if !self.is_either(j) {
                    let new_mask = self.n_buckets - 1;
                    self.set_is_del_true(j);
                    let mut key = unsafe {
                        let mut k: mem::MaybeUninit<K> = mem::MaybeUninit::uninit();
                        ptr::copy_nonoverlapping(self.keys.add(j as usize), k.as_mut_ptr(), 1);
                        k.assume_init_read()
                    };

                    let mut val = val_ptr.as_ref().map(|vptr| unsafe {
                        let mut v: mem::MaybeUninit<V> = mem::MaybeUninit::uninit();
                        ptr::copy_nonoverlapping((*vptr).add(j as usize), v.as_mut_ptr(), 1);
                        (v.assume_init_read(), **vptr)
                    });
                    loop {
                        let mut step = 0;
                        let k = K::hash(&key);
                        let mut i = k & new_mask;
                        while !is_empty(new_flags, i) {
                            step += 1;
                            i = (i + step) & new_mask;
                        }
                        set_is_empty_false(new_flags, i);
                        if i < nb && !is_either(self.flags, i) {
                            // Kick out the existing element
                            unsafe { ptr::swap(self.keys.add(i as usize), &mut key) };

                            // Same for values if this is a HashMap
                            if let Some((mut p, mut p1)) = val.take() {
                                unsafe { ptr::swap(p1.add(i as usize), &mut p) };
                                val = Some((p, p1))
                            }
                            // Mark as deleted in old hash table
                            self.set_is_del_true(i);
                        } else {
                            // Write the element and break out of the loop
                            unsafe { ptr::copy_nonoverlapping(&key, self.keys.add(i as usize), 1) }
                            if let Some((p, p1)) = val.take() {
                                unsafe { ptr::copy_nonoverlapping(&p, p1.add(i as usize), 1) }
                            }
                            break;
                        }
                    }
                    if nb > new_n_buckets {
                        // Shrink the hash table
                        self.keys = unsafe {
                            libc::realloc(
                                self.keys as *mut c_void,
                                (new_n_buckets as size_t) * mem::size_of::<K>(),
                            )
                        } as *mut K;
                        if let Some(vptr) = val_ptr.as_mut() {
                            **vptr = unsafe {
                                libc::realloc(
                                    self.keys as *mut c_void,
                                    (new_n_buckets as size_t) * mem::size_of::<V>(),
                                )
                            } as *mut V;
                        }
                    }
                    unsafe { libc::free(self.flags as *mut c_void) }
                    self.flags = new_flags;
                    self.n_buckets = new_n_buckets;
                    self.n_occupied = self.size;
                    self.upper_bound = ((self.n_buckets as f64) * HASH_UPPER).round() as KHInt;
                }
            }
        }
        Ok(())
    }
}

#[repr(C)]
pub struct KHashMapRaw<K, V> {
    hash: KHashRaw<K>,
    vals: *mut V,
}

impl<K, V> Deref for KHashMapRaw<K, V> {
    type Target = KHashRaw<K>;

    fn deref(&self) -> &Self::Target {
        unsafe { &self.hash }
    }
}

impl<K, V> DerefMut for KHashMapRaw<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut self.hash }
    }
}

impl<K, V> KHashMapRaw<K, V> {
    fn free(&mut self) {
        self.hash.free();
        unsafe { libc::free(self.vals as *mut c_void) };
        self.vals = ptr::null_mut();
    }
    #[inline]
    unsafe fn get_val_unchecked(&self, i: u32) -> &V {
        &*self.vals.add(i as usize)
    }
    #[inline]
    pub fn get_val(&self, i: u32) -> Option<&V> {
        if i < self.n_buckets {
            Some(unsafe { &*self.vals.add(i as usize) })
        } else {
            None
        }
    }
}
#[repr(C)]
pub struct KHashSetRaw<K> {
    hash: KHashRaw<K>,
    _unused: *mut c_void, // Unused pointer (should be null)
}

impl<K> Deref for KHashSetRaw<K> {
    type Target = KHashRaw<K>;

    fn deref(&self) -> &Self::Target {
        unsafe { &self.hash }
    }
}

impl<K> DerefMut for KHashSetRaw<K> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut self.hash }
    }
}

pub struct KHashMap<'a, K, V> {
    inner: *mut KHashMapRaw<K, V>,
    phantom: PhantomData<&'a KHashMapRaw<K, V>>,
}

impl<'a, K, V> Deref for KHashMap<'a, K, V> {
    type Target = KHashMapRaw<K, V>;

    fn deref(&self) -> &Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { &*self.inner }
    }
}

impl<'a, K, V> DerefMut for KHashMap<'a, K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { &mut *self.inner }
    }
}

impl<'a, K, V> Drop for KHashMap<'a, K, V> {
    fn drop(&mut self) {
        self.free();
        unsafe {
            libc::free(self.inner as *mut c_void);
        }
    }
}

impl<'a, K: KHashFunc + PartialEq, V: Default> KHashMap<'a, K, V> {
    pub fn init() -> Self {
        let inner = unsafe { libc::calloc(1, mem::size_of::<Self>()) as *mut KHashMapRaw<K, V> };
        assert!(!inner.is_null(), "Out of memory error");
        Self {
            inner,
            phantom: PhantomData,
        }
    }
}

pub struct KHashSet<'a, K> {
    inner: *mut KHashSetRaw<K>,
    phantom: PhantomData<&'a KHashSetRaw<K>>,
}

impl<'a, K> Deref for KHashSet<'a, K> {
    type Target = KHashSetRaw<K>;

    fn deref(&self) -> &Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { &*self.inner }
    }
}

impl<'a, K> DerefMut for KHashSet<'a, K> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // We can do this safely as self.inner is always non-null
        unsafe { &mut *self.inner }
    }
}

impl<'a, K> Drop for KHashSet<'a, K> {
    fn drop(&mut self) {
        self.free();
        unsafe {
            libc::free(self.inner as *mut c_void);
        }
    }
}
