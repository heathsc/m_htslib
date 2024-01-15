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
    _get_flag(flags, i) >> ((i & 0xf) << 1)
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
        if i < self.n_buckets && !self.is_either(i) {
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
    fn _find(&self, key: &K) -> Option<KHInt> {
        if self.n_buckets > 0 {
            let mut step = 0;
            let mask = self.n_buckets - 1;
            let k = K::hash(key);
            let mut i = k & mask;
            let last = i;
            while !self.is_empty(i)
                && (self.is_del(i) || key != unsafe { self.get_key_unchecked(i) })
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
    fn _find_entry<V>(&mut self, key: &K, vptr: Option<&mut *mut V>) -> Result<KHInt, KHashError> {
        eprintln!("find_entry: {} {}", self.n_occupied, self.upper_bound);
        if self.n_occupied >= self.upper_bound {
            eprintln!("Resizing");
            // Update hash table
            if self.n_buckets > (self.size << 1) {
                // Clear "deleted" elements
                self.resize(self.n_buckets - 1, vptr)?;
            } else {
                // Expand hash table
                self.resize(self.n_buckets + 1, vptr)?;
            }
            eprintln!("n_buckets now {}", self.n_buckets);
        }
        let mask = self.n_buckets - 1;
        let k = K::hash(key);
        let mut i = k & mask;
        eprintln!("k = {k}, mask = {mask}, i = {i}");
        let x = if self.is_empty(i) {
            eprintln!("Empty!");
            i // for speed up
        } else {
            let mut site = self.n_buckets;
            let mut x = site;
            let last = i;
            let mut step = 0;
            while !self.is_empty(i)
                && (self.is_del(i) || key != unsafe { self.get_key_unchecked(i) })
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
                if !self.is_either(j) {
                    let new_mask = new_n_buckets - 1;
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
        if i < self.n_buckets && !self.is_either(i) {
            Some(unsafe { &*self.vals.add(i as usize) })
        } else {
            None
        }
    }
}

impl<K: KHashFunc + PartialEq, V> KHashMapRaw<K, V> {
    #[inline]
    pub fn entry(&mut self, key: K) -> Result<MapEntryMut<K, V>, KHashError> {
        self.hash
            ._find_entry(&key, Some(&mut self.vals))
            .map(|idx| MapEntryMut {
                map: self,
                idx,
                key,
            })
    }
    #[inline]
    pub fn find(&self, key: &K) -> Option<MapEntry<K, V>> {
        self._find(key).map(|idx| MapEntry { map: self, idx })
    }
    #[inline]
    pub fn insert(&mut self, key: K, val: V) -> Result<Option<V>, KHashError> {
        let idx = self.hash._find_entry(&key, Some(&mut self.vals))?;
        Ok(_insert_val(self, idx, key, val))
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
        let inner = unsafe {
            libc::calloc(1, mem::size_of::<KHashMapRaw<K, V>>()) as *mut KHashMapRaw<K, V>
        };
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

pub struct MapEntry<'a, K, V> {
    map: &'a KHashMapRaw<K, V>,
    idx: KHInt,
}

impl<'a, K, V> MapEntry<'a, K, V> {
    #[inline]
    pub fn idx(&self) -> KHInt {
        self.idx
    }

    #[inline]
    pub fn value(&self) -> Option<&V> {
        self.map.get_val(self.idx)
    }

    #[inline]
    pub fn key(&self) -> Option<&K> {
        self.map.get_key(self.idx)
    }
}

pub struct MapEntryMut<'a, K, V> {
    map: &'a mut KHashMapRaw<K, V>,
    key: K,
    idx: KHInt,
}

impl<'a, K, V> MapEntryMut<'a, K, V> {
    #[inline]
    pub fn idx(&self) -> KHInt {
        self.idx
    }

    #[inline]
    pub fn insert(mut self, mut val: V) -> Option<V> {
        let i = self.idx;
        assert!(i < self.map.n_buckets);
        _insert_val(self.map, i, self.key, val)
    }
}

fn _insert_val<K, V>(map: &mut KHashMapRaw<K, V>, i: KHInt, key: K, mut val: V) -> Option<V> {
    let fg = get_flag(map.flags, i);
    eprintln!("Inserting: i={i}, fg={}", fg & 3);
    if (fg & 3) != 0 {
        // Either not present or deleted
        unsafe {
            ptr::copy_nonoverlapping(&key, map.keys.add(i as usize), 1);
            ptr::copy_nonoverlapping(&val, map.vals.add(i as usize), 1);
        }
        map.size += 1;
        if (fg & 2) != 0 {
            map.n_occupied += 1;
        }
        set_is_both_false(map.flags, i);
        None
    } else {
        unsafe { ptr::swap(&mut val, map.vals.add(i as usize)) }
        Some(val)
    }
}

impl KHashFunc for KHInt {
    fn hash(&self) -> u32 {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn hash_int_cstr() -> Result<(), KHashError> {
        let mut h: KHashMap<KHInt, &CStr> = KHashMap::init();
        assert_eq!(h.insert(10, c"String10")?, None);
        assert_eq!(h.insert(2, c"String2")?, None);
        assert_eq!(h.insert(290, c"String290")?, None);
        assert_eq!(h.insert(2, c"String2a")?, Some(c"String2"));
        assert_eq!(h.insert(500, c"String500")?, None);
        assert_eq!(h.insert(20, c"String20")?, None);

        let m = h.find(&2).expect("Missing entry");
        assert_eq!(m.value(), Some(&c"String2a"));

        assert_eq!(h.insert(1, c"String1")?, None);
        assert_eq!(h.insert(100, c"String100")?, None);
        assert_eq!(h.insert(7, c"String7")?, None);
        assert_eq!(h.insert(98, c"String98")?, None);
        assert_eq!(h.insert(16384, c"String16384")?, None);

        let m = h.find(&10).expect("Missing entry");
        assert_eq!(m.value(), Some(&c"String10"));

        Ok(())
    }
}
