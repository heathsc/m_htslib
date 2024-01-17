use std::{
    fmt::Debug,
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
    ptr,
    str::FromStr,
};

use libc::c_void;

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
        unsafe { &self.hash }
    }
}

impl<K> DerefMut for KHashSetRaw<K> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut self.hash }
    }
}

impl<K> KHashSetRaw<K> {
    fn free(&mut self) {
        // Drop all keys and values
        for i in 0..self.n_buckets() {
            if !self.is_either(i) {
                unsafe {
                    self._drop_key(i);
                }
            }
        }
        self._clear();
        self.hash.free();
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
                ptr::write(self.keys().add(idx as usize), key);
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
        self.free();
        unsafe {
            libc::free(self.inner as *mut c_void);
        }
    }
}

impl<'a, K: KHashFunc + PartialEq> KHashSet<'a, K> {
    pub fn init() -> Self {
        let inner =
            unsafe { libc::calloc(1, mem::size_of::<KHashSetRaw<K>>()) as *mut KHashSetRaw<K> };
        assert!(!inner.is_null(), "Out of memory error");
        Self {
            inner,
            phantom: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_int() -> Result<(), KHashError> {
        let mut h = KHashSet::init();
        assert_eq!(h.insert(42u32)?, false);
        assert_eq!(h.insert(64)?, false);
        assert_eq!(h.insert(1)?, false);
        assert_eq!(h.insert(73)?, false);
        assert_eq!(h.insert(1024)?, false);
        assert_eq!(h.insert(64)?, true);
        eprintln!("Removing key 1");
        assert_eq!(h.delete(&1), true);
        assert_eq!(h.insert(1)?, false);
        Ok(())
    }

    #[test]
    fn set_str() -> Result<(), KHashError> {
        let mut h = KHashSet::init();
        assert_eq!(h.insert("key1")?, false);
        assert_eq!(h.insert("key2")?, false);
        assert_eq!(h.insert("key1")?, true);
        Ok(())
    }
}
