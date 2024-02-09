use std::{
    fmt::Debug,
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
    ptr,
    str::FromStr,
};

use libc::{c_void, size_t};

use super::*;
use crate::KHashError;

#[repr(C)]
pub struct KHashSetRaw<K> {
    hash: KHashRaw<K>,
    _unused: *mut c_void, // Unused pointer (should be null)
}

impl<K> Deref for KHashSetRaw<K> {
    type Target = KHashRaw<K>;

    fn deref(&self) -> &Self::Target {
        &self.hash
    }
}

impl<K> DerefMut for KHashSetRaw<K> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.hash
    }
}

impl<K> KHashSetRaw<K> {
    fn free(&mut self) {
        // Drop all keys and values
        for i in 0..self.n_buckets() {
            if !self.is_bin_either(i) {
                unsafe {
                    self._drop_key(i);
                }
            }
        }
        self._clear();
        self.hash.free();
    }
    #[inline]
    pub fn iter(&self) -> KIter<K> {
        self.hash.keys()
    }
}

impl<K: KHashFunc + PartialEq> KHashSetRaw<K> {
    #[inline]
    pub fn find(&self, key: &K) -> Option<KHInt> {
        self._find(key)
    }

    pub fn insert(&mut self, key: K) -> Result<bool, KHashError> {
        let n: Option<&mut *mut u8> = None; // Dummy just to get the write annotation for V
        let idx = self.hash._find_entry(&key, n)?;
        let fg = get_flag(self.flags(), idx);
        Ok(if (fg & 3) != 0 {
            // Either not present or deleted
            unsafe {
                ptr::write(self.keys_ptr_mut().add(idx as usize), key);
            }
            self.inc_size();
            if (fg & 2) != 0 {
                self.inc_n_occupied();
            }
            set_is_both_false(self.flags(), idx);
            false
        } else {
            true
        })
    }

    pub fn delete(&mut self, key: &K) -> bool {
        self._find(key)
            .map(|idx| {
                self._del(idx);
                true
            })
            .unwrap_or(false)
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
        if !self.inner.is_null() {
            // Drop inner
            let _ = unsafe { ptr::read(self.inner) };
            unsafe {
                libc::free(self.inner as *mut c_void);
            }
        }
    }
}

impl<'a, K> Default for KHashSet<'a, K> {
    fn default() -> Self {
        let inner =
            unsafe { libc::calloc(1, mem::size_of::<KHashSetRaw<K>>()) as *mut KHashSetRaw<K> };
        assert!(!inner.is_null(), "Out of memory error");
        Self {
            inner,
            phantom: PhantomData,
        }
    }
}
impl<'a, K> KHashSet<'a, K> {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Consumes and leaks the [KHashSet], returning a mutable reference to [KHashRaw].
    /// After calling this function, the [KHashRaw] structure (containing the keys, values etc.)
    /// will not be automatically deallocated when the reference is dropped.  If the keys and values
    /// are allocated on the heap and do not require special deallocation then the htslib function
    /// kh_destroy() can be used (and this is the only way by which a [KHashSet] instance created in
    /// Rust can be safely passed to kh_destroy()).  Otherwise, [KHashSet::from_raw_ptr] can be used recreate
    /// a valid [KHashSet] instance which will handle correctly the deallocation.
    pub fn leak(self) -> &'a mut KHashSetRaw<K> {
        let mut me = mem::ManuallyDrop::new(self);
        unsafe { &mut *me.inner }
    }

    /// Make new [KHashSet] instance from raw pointer to [KHashSetRaw]. Note that the
    /// generic type `K` for the Key type must be correctly specified.
    ///
    /// # Safety:
    ///
    /// `inner` must be a valid, correctly aligned, pointer to a unique, initialized KHashMapRaw struct
    /// (either initialized from Rust or from C) with the correct type `K` for the Keys.  
    /// In particular, the pointer must be the only existing pointer to the KHashSetRaw structure,
    /// otherwise the internal structures will be freed twice.
    pub unsafe fn from_raw_ptr(inner: *mut KHashSetRaw<K>) -> Self {
        assert!(
            !inner.is_null(),
            "KHashMap::from_raw_ptr called with null pointer"
        );
        Self {
            inner,
            phantom: PhantomData,
        }
    }
    #[inline]
    pub fn into_keys(mut self) -> KIntoKeys<K> {
        let map = unsafe { ptr::read(&self.hash) };
        self.inner = ptr::null_mut();
        map.into_keys()
    }
}
impl<'a, K: KHashFunc + PartialEq> KHashSet<'a, K> {
    pub fn with_capacity(sz: KHInt) -> Self {
        let mut h = Self::default();
        h.expand(sz);
        h
    }
}

impl<'a, K> IntoIterator for &KHashSet<'a, K> {
    type Item = &'a K;
    type IntoIter = KIter<'a, K>;

    fn into_iter(self) -> Self::IntoIter {
        KIter::make(&self.hash as *const KHashRaw<K>)
    }
}

impl<'a, K> IntoIterator for KHashSet<'a, K> {
    type Item = K;
    type IntoIter = KIntoKeys<K>;

    fn into_iter(self) -> Self::IntoIter {
        self.into_keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_int() -> Result<(), KHashError> {
        let mut h = KHashSet::new();
        assert_eq!(h.insert(42u32)?, false);
        assert_eq!(h.insert(64)?, false);
        assert_eq!(h.insert(1)?, false);
        assert_eq!(h.insert(73)?, false);
        assert_eq!(h.insert(1024)?, false);
        assert_eq!(h.insert(64)?, true);
        eprintln!("Removing key 1");
        assert_eq!(h.delete(&1), true);
        assert_eq!(h.insert(1)?, false);

        // Test keys iterator
        assert_eq!(h.iter().nth(1), Some(&1));

        // Test drain iterator
        assert_eq!(h.drain().nth(1), Some(1));
        assert!(h.is_empty());

        Ok(())
    }

    #[test]
    fn set_str() -> Result<(), KHashError> {
        let mut h = KHashSet::new();
        assert_eq!(h.insert("key1")?, false);
        assert_eq!(h.insert("key2")?, false);
        assert_eq!(h.insert("key1")?, true);

        for k in &h {
            eprintln!("{}", k);
        }

        let rf = h.leak();
        let mut h1 = unsafe { KHashSet::from_raw_ptr(rf) };

        for k in h1 {
            eprintln!("{}", k);
        }

        Ok(())
    }
}
