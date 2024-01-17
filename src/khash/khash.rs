use std::{fmt::Debug, mem, ptr};

use super::khash_func::*;
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

#[inline]
fn _get_flag(flags: *const u32, i: u32) -> u32 {
    unsafe { *flags.add((i as usize) >> 4) }
}
#[inline]
pub(super) fn get_flag(flags: *const u32, i: u32) -> u32 {
    _get_flag(flags, i) >> ((i & 0xf) << 1)
}
#[inline]
fn get_flag_ptr(flags: *mut u32, i: u32) -> *mut u32 {
    unsafe { flags.add((i as usize) >> 4) }
}
#[inline]
pub(super) fn is_del(flags: *const u32, i: u32) -> bool {
    (get_flag(flags, i) & 1) != 0
}
#[inline]
pub(super) fn is_empty(flags: *const u32, i: u32) -> bool {
    (get_flag(flags, i) & 2) != 0
}
#[inline]
pub(super) fn is_either(flags: *const u32, i: u32) -> bool {
    (get_flag(flags, i) & 3) != 0
}

#[inline]
pub(super) fn set_is_del_false(flags: *mut u32, i: u32) {
    unsafe { *get_flag_ptr(flags, i) &= !(1 << ((i & 0xf) << 1)) }
}
#[inline]
pub(super) fn set_is_empty_false(flags: *mut u32, i: u32) {
    unsafe { *get_flag_ptr(flags, i) &= !(2 << ((i & 0xf) << 1)) }
}
#[inline]
pub(super) fn set_is_both_false(flags: *mut u32, i: u32) {
    unsafe { *get_flag_ptr(flags, i) &= !(3 << ((i & 0xf) << 1)) }
}
#[inline]
pub(super) fn set_is_del_true(flags: *mut u32, i: u32) {
    unsafe { *get_flag_ptr(flags, i) |= 1 << ((i & 0xf) << 1) }
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

impl<K> KHashRaw<K> {
    #[inline]
    pub(super) unsafe fn get_key_unchecked(&self, i: u32) -> &K {
        &*self.keys.add(i as usize)
    }

    #[inline]
    pub fn get_key(&self, i: u32) -> Option<&K> {
        if i < self.n_buckets && !self.is_bin_either(i) {
            Some(unsafe { &*self.keys.add(i as usize) })
        } else {
            None
        }
    }
    #[inline]
    pub(super) fn is_bin_del(&self, i: u32) -> bool {
        is_del(self.flags, i)
    }
    #[inline]
    pub(super) fn is_bin_empty(&self, i: u32) -> bool {
        is_empty(self.flags, i)
    }
    #[inline]
    pub(super) fn is_bin_either(&self, i: u32) -> bool {
        is_either(self.flags, i)
    }
    #[inline]
    pub(super) fn set_is_bin_del_false(&mut self, i: u32) {
        set_is_del_false(self.flags, i)
    }
    #[inline]
    pub(super) fn set_is_bin_empty_false(&mut self, i: u32) {
        set_is_empty_false(self.flags, i)
    }
    #[inline]
    pub(super) fn set_is_bin_both_false(&mut self, i: u32) {
        set_is_both_false(self.flags, i)
    }
    #[inline]
    pub(super) fn set_is_bin_del_true(&mut self, i: u32) {
        set_is_del_true(self.flags, i)
    }

    #[inline]
    pub(super) fn free(&mut self) {
        unsafe {
            libc::free(self.flags as *mut c_void);
            self.flags = ptr::null_mut();
            libc::free(self.keys as *mut c_void);
            self.keys = ptr::null_mut();
        }
    }

    // Note, you must drop keys (and values for a Map) before doing this otherwise memory will be leaked
    pub(super) fn _clear(&mut self) {
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

    #[inline]
    pub(super) unsafe fn _drop_key(&mut self, i: KHInt) -> K {
        ptr::read(self.keys.add(i as usize))
    }

    // Deletes a key from the hash

    #[inline]
    pub(super) fn _del(&mut self, x: KHInt) {
        if x < self.n_buckets && !self.is_bin_either(x) {
            unsafe {
                let _ = self._drop_key(x);
            }
            self.set_is_bin_del_true(x);
            assert!(self.size > 0);
            self.size -= 1;
        }
    }
    #[inline]
    pub(super) fn n_buckets(&self) -> KHInt {
        self.n_buckets
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.n_occupied == 0
    }
    #[inline]
    pub fn len(&self) -> KHInt {
        self.n_occupied
    }
    #[inline]
    pub(super) fn flags(&mut self) -> *mut u32 {
        self.flags
    }

    #[inline]
    pub(super) fn keys(&self) -> *const K {
        self.keys
    }

    #[inline]
    pub(super) fn keys_mut(&mut self) -> *mut K {
        self.keys
    }

    #[inline]
    pub(super) fn inc_size(&mut self) {
        self.size += 1
    }

    #[inline]
    pub(super) fn inc_n_occupied(&mut self) {
        self.n_occupied += 1
    }
}

impl<K: KHashFunc + PartialEq> KHashRaw<K> {
    pub(super) fn _find(&self, key: &K) -> Option<KHInt> {
        if self.n_buckets > 0 {
            let mut step = 0;
            let mask = self.n_buckets - 1;
            let k = K::hash(key);
            let mut i = k & mask;
            let last = i;
            while !self.is_bin_empty(i)
                && (self.is_bin_del(i) || key != unsafe { self.get_key_unchecked(i) })
            {
                step += 1;
                i = (i + step) & mask;
                if i == last {
                    return None;
                }
            }
            if self.is_bin_either(i) {
                None
            } else {
                Some(i)
            }
        } else {
            None
        }
    }
    #[inline]
    pub fn exists(&self, key: &K) -> bool {
        self._find(key).is_some()
    }
    pub(super) fn _find_entry<V>(
        &mut self,
        key: &K,
        vptr: Option<&mut *mut V>,
    ) -> Result<KHInt, KHashError> {
        if self.n_occupied >= self.upper_bound {
            // Update hash table
            if self.n_buckets > (self.size << 1) {
                // Clear "deleted" elements
                self.resize(self.n_buckets - 1, vptr)?;
            } else {
                // Expand hash table
                self.resize(self.n_buckets + 1, vptr)?;
            }
        }
        let mask = self.n_buckets - 1;
        let k = K::hash(key);
        let mut i = k & mask;
        let x = if self.is_bin_empty(i) {
            i // for speed up
        } else {
            let mut site = self.n_buckets;
            let mut x = site;
            let last = i;
            let mut step = 0;
            while !self.is_bin_empty(i)
                && (self.is_bin_del(i) || key != unsafe { self.get_key_unchecked(i) })
            {
                if self.is_bin_del(i) {
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
                if self.is_bin_empty(i) && site != self.n_buckets {
                    x = site
                } else {
                    x = i
                }
            }
            x
        };
        Ok(x)
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
                if !self.is_bin_either(j) {
                    let new_mask = new_n_buckets - 1;
                    self.set_is_bin_del_true(j);
                    let mut key = unsafe { ptr::read(self.keys.add(j as usize)) };

                    let mut val = val_ptr.as_ref().map(|vptr| unsafe {
                        let v = ptr::read((*vptr).add(j as usize));
                        (v, **vptr)
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
                            self.set_is_bin_del_true(i);
                        } else {
                            // Write the element and break out of the loop
                            unsafe { ptr::write(self.keys.add(i as usize), key) }
                            if let Some((p, p1)) = val.take() {
                                unsafe { ptr::write(p1.add(i as usize), p) }
                            }
                            break;
                        }
                    }
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
        Ok(())
    }
}
