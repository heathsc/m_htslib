use std::{
    fmt::Debug,
    iter::FusedIterator,
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
    ptr,
    str::FromStr,
};

use libc::c_void;

use super::*;
use crate::{kstring::KString, KHashError};

#[repr(C)]
pub struct KHashMapRaw<K, V> {
    hash: KHashRaw<K>,
    vals: *mut V,
}

impl<K, V> Deref for KHashMapRaw<K, V> {
    type Target = KHashRaw<K>;

    fn deref(&self) -> &Self::Target {
        &self.hash
    }
}

impl<K, V> DerefMut for KHashMapRaw<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.hash
    }
}

impl<K, V> Drop for KHashMapRaw<K, V> {
    fn drop(&mut self) {
        self.free_vals()
    }
}

impl<K, V> KHashMapRaw<K, V> {
    fn free_vals(&mut self) {
        if !self.vals.is_null() {
            self.drop_vals();
            unsafe { libc::free(self.vals as *mut c_void) };
            self.vals = ptr::null_mut();
        }
    }
    fn drop_vals(&mut self) {
        for i in 0..self.n_buckets() {
            if !self.is_bin_either(i) {
                unsafe {
                    let _ = self._drop_val(i);
                }
            }
        }
    }
    #[inline]
    unsafe fn get_val_unchecked(&self, i: u32) -> &V {
        &*self.vals.add(i as usize)
    }

    #[inline]
    unsafe fn get_val_unchecked_mut(&mut self, i: u32) -> &mut V {
        &mut *self.vals.add(i as usize)
    }

    #[inline]
    pub fn get_val(&self, i: u32) -> Option<&V> {
        if i < self.n_buckets() && !self.is_bin_either(i) {
            Some(unsafe { &*self.vals.add(i as usize) })
        } else {
            None
        }
    }
    #[inline]
    unsafe fn _drop_val(&mut self, i: KHInt) -> V {
        ptr::read(self.vals.add(i as usize))
    }
    #[inline]
    pub fn iter(&self) -> KIterMap<K, V> {
        KIterMap {
            map: self as *const KHashMapRaw<K, V>,
            idx: 0,
            phantom: PhantomData,
        }
    }
    #[inline]
    pub fn iter_mut(&mut self) -> KIterMapMut<K, V> {
        KIterMapMut {
            map: self as *mut KHashMapRaw<K, V>,
            idx: 0,
            phantom: PhantomData,
        }
    }
    #[inline]
    pub fn drain(&mut self) -> KDrainMap<K, V> {
        KDrainMap {
            inner: self.iter_mut(),
        }
    }
    #[inline]
    pub fn values(&self) -> KIterVal<K, V> {
        KIterVal { inner: self.iter() }
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
    pub fn get(&self, key: &K) -> Option<&V> {
        self._find(key)
            .map(|idx| unsafe { self.get_val_unchecked(idx) })
    }

    #[inline]
    pub fn insert(&mut self, key: K, val: V) -> Result<Option<V>, KHashError> {
        let idx = self.hash._find_entry(&key, Some(&mut self.vals))?;
        Ok(_insert_val(self, idx, key, val))
    }

    #[inline]
    pub fn delete(&mut self, key: &K) -> Option<V> {
        self._find(key).map(|idx| {
            self._del(idx);
            unsafe { self._drop_val(idx) }
        })
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
        if !self.inner.is_null() {
            // Drop inner
            let _ = unsafe { ptr::read(self.inner) };
            unsafe {
                libc::free(self.inner as *mut c_void);
            }
        }
    }
}

impl<'a, K, V> KHashMap<'a, K, V> {
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
    #[inline]
    pub fn into_keys(mut self) -> KIntoKeys<K> {
        let khash = unsafe { ptr::read(&self.hash) };
        self.free_vals();
        self.inner = ptr::null_mut();
        khash.into_keys()
    }

    #[inline]
    pub fn into_values(mut self) -> KIntoValues<K, V> {
        let map = unsafe { ptr::read(self.inner) };
        self.inner = ptr::null_mut();
        KIntoValues { map, idx: 0 }
    }
    #[inline]
    pub fn into_iter(mut self) -> KIntoIter<K, V> {
        let map = unsafe { ptr::read(self.inner) };
        self.inner = ptr::null_mut();
        KIntoIter { map, idx: 0 }
    }
}

impl<'a, K, V> IntoIterator for &KHashMap<'a, K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = KIterMap<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        KIterMap {
            map: self.inner,
            idx: 0,
            phantom: PhantomData,
        }
    }
}

impl<'a, K, V> IntoIterator for &mut KHashMap<'a, K, V> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = KIterMapMut<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        KIterMapMut {
            map: self.inner,
            idx: 0,
            phantom: PhantomData,
        }
    }
}

impl<'a, K, V> IntoIterator for KHashMap<'a, K, V> {
    type Item = (K, V);
    type IntoIter = KIntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.into_iter()
    }
}

pub struct KIterMap<'a, K, V> {
    map: *const KHashMapRaw<K, V>,
    idx: KHInt,
    phantom: PhantomData<&'a KHashMapRaw<K, V>>,
}

impl<'a, K, V> KIterMap<'a, K, V> {
    #[inline]
    unsafe fn as_ref(&self) -> &'a KHashMapRaw<K, V> {
        {
            &*self.map
        }
    }
}
impl<'a, K, V> Iterator for KIterMap<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        let map = unsafe { self.as_ref() };
        let nb = map.n_buckets();
        let mut x = None;

        while self.idx < nb && x.is_none() {
            let empty = map.is_bin_either(self.idx);
            if !empty {
                unsafe {
                    let k = map.get_key_unchecked(self.idx);
                    let v = map.get_val_unchecked(self.idx);
                    x = Some((k, v))
                }
            }
            self.idx += 1;
        }
        x
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        unsafe { self.as_ref() }.size_hint()
    }
}

impl<'a, K, V> ExactSizeIterator for KIterMap<'a, K, V> {}
impl<'a, K, V> FusedIterator for KIterMap<'a, K, V> {}

pub struct KIterMapMut<'a, K, V> {
    map: *mut KHashMapRaw<K, V>,
    idx: KHInt,
    phantom: PhantomData<&'a mut KHashMapRaw<K, V>>,
}

impl<'a, K, V> KIterMapMut<'a, K, V> {
    #[inline]
    unsafe fn as_ref(&self) -> &'a KHashMapRaw<K, V> {
        &*self.map
    }

    #[inline]
    unsafe fn as_mut(&mut self) -> &'a mut KHashMapRaw<K, V> {
        &mut *self.map
    }
}

impl<'a, K, V> Iterator for KIterMapMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        let map = unsafe { self.as_mut() };
        let keys = map.keys_ptr();
        let nb = map.n_buckets();
        let mut x = None;

        while self.idx < nb {
            let empty = map.is_bin_either(self.idx);
            if !empty {
                unsafe {
                    let k = &*keys.add(self.idx as usize);
                    let v = map.get_val_unchecked_mut(self.idx);
                    x = Some((k, v));
                    self.idx += 1;
                    break;
                }
            }
            self.idx += 1;
        }
        x
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        unsafe { self.as_ref().size_hint() }
    }
}

impl<'a, K, V> ExactSizeIterator for KIterMapMut<'a, K, V> {}
impl<'a, K, V> FusedIterator for KIterMapMut<'a, K, V> {}

pub struct KIterVal<'a, K, V> {
    inner: KIterMap<'a, K, V>,
}

impl<'a, K, V> Iterator for KIterVal<'a, K, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        let map = unsafe { self.inner.as_ref() };
        let nb = map.n_buckets();
        let mut x = None;

        while self.inner.idx < nb && x.is_none() {
            let empty = map.is_bin_either(self.inner.idx);
            if !empty {
                unsafe {
                    let v = map.get_val_unchecked(self.inner.idx);
                    x = Some(v)
                }
            }
            self.inner.idx += 1;
        }
        x
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        unsafe { self.inner.as_ref() }.size_hint()
    }
}

impl<'a, K, V> ExactSizeIterator for KIterVal<'a, K, V> {}
impl<'a, K, V> FusedIterator for KIterVal<'a, K, V> {}

pub struct KIntoIter<K, V> {
    map: KHashMapRaw<K, V>,
    idx: KHInt,
}

impl<K, V> Iterator for KIntoIter<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        let map = &mut self.map;
        let nb = map.n_buckets();
        let keys = map.keys_ptr();
        let mut x = None;

        while self.idx < nb {
            let empty = map.is_bin_either(self.idx);
            if !empty {
                // Drop key
                let k = unsafe { ptr::read(keys.add(self.idx as usize)) };
                // Take value
                let v = unsafe { ptr::read(map.vals.add(self.idx as usize)) };
                x = Some((k, v));
                map.set_is_bin_del_true(self.idx);
                self.idx += 1;
                break;
            }
            self.idx += 1;
        }
        x
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.map.size_hint()
    }
}

impl<K, V> ExactSizeIterator for KIntoIter<K, V> {}
impl<K, V> FusedIterator for KIntoIter<K, V> {}

pub struct KIntoValues<K, V> {
    map: KHashMapRaw<K, V>,
    idx: KHInt,
}

impl<K, V> Iterator for KIntoValues<K, V> {
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        let map = &mut self.map;
        let nb = map.n_buckets();
        let keys = map.keys_ptr();
        let mut x = None;

        while self.idx < nb {
            let empty = map.is_bin_either(self.idx);
            if !empty {
                // Drop key
                let _ = unsafe { ptr::read(keys.add(self.idx as usize)) };
                // Take value
                x = Some(unsafe { ptr::read(map.vals.add(self.idx as usize)) });
                map.set_is_bin_del_true(self.idx);
                self.idx += 1;
                break;
            }
            self.idx += 1;
        }
        x
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.map.size_hint()
    }
}

impl<K, V> ExactSizeIterator for KIntoValues<K, V> {}
impl<K, V> FusedIterator for KIntoValues<K, V> {}

pub struct KDrainMap<'a, K, V> {
    inner: KIterMapMut<'a, K, V>,
}

impl<'a, K, V> Iterator for KDrainMap<'a, K, V> {
    type Item = (K, V);
    fn next(&mut self) -> Option<Self::Item> {
        let map = unsafe { self.inner.as_mut() };
        let keys = map.keys_ptr();
        let nb = map.n_buckets();
        let mut x = None;

        while self.inner.idx < nb {
            let empty = map.is_bin_either(self.inner.idx);
            if !empty {
                let k = unsafe { ptr::read(keys.add(self.inner.idx as usize)) };
                let v = unsafe { ptr::read(map.vals.add(self.inner.idx as usize)) };
                map.set_is_bin_del_true(self.inner.idx);
                x = Some((k, v));
                self.inner.idx += 1;
                break;
            }
            self.inner.idx += 1;
        }
        x
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, K, V> ExactSizeIterator for KDrainMap<'a, K, V> {}
impl<'a, K, V> FusedIterator for KDrainMap<'a, K, V> {}

impl<'a, K, V> Drop for KDrainMap<'a, K, V> {
    fn drop(&mut self) {
        let map = unsafe { self.inner.as_mut() };
        map.drop_vals();
        map._drop_keys();
        map._clear();
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
    pub fn insert(self, val: V) -> Option<V> {
        let i = self.idx;
        assert!(i < self.map.n_buckets());
        _insert_val(self.map, i, self.key, val)
    }
}

fn _insert_val<K, V>(map: &mut KHashMapRaw<K, V>, i: KHInt, key: K, mut val: V) -> Option<V> {
    let fg = get_flag(map.flags(), i);
    if (fg & 3) != 0 {
        // Either not present or deleted
        unsafe {
            ptr::write(map.keys_ptr_mut().add(i as usize), key);
            ptr::write(map.vals.add(i as usize), val);
        }
        map.inc_size();
        if (fg & 2) != 0 {
            map.inc_n_occupied();
        }
        set_is_both_false(map.flags(), i);
        None
    } else {
        unsafe { ptr::swap(&mut val, map.vals.add(i as usize)) }
        Some(val)
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

        assert_eq!(h.get(&10), Some(&c"String10"));

        // Test iterator
        let (k, v) = h.iter().nth(5).unwrap();
        assert_eq!((*k, *v), (20, c"String20"));

        // Test mutable iterator
        let (_, v) = h.iter_mut().nth(5).unwrap();
        *v = c"String20_changed";
        assert_eq!(h.get(&20), Some(&c"String20_changed"));

        // Test values iterator
        let v = h.values().nth(5);
        assert_eq!(v, Some(&c"String20_changed"));

        // Test delete
        assert_eq!(h.len(), 10);
        let v = h.delete(&20).unwrap();
        assert_eq!(v, c"String20_changed");
        assert_eq!(h.len(), 9);
        assert_eq!(h.get(&20), None);

        // Test drain iterator
        let (v, _) = h.drain().nth(3).unwrap();
        assert_eq!(v, 2);

        // Hash is empty after drain
        assert!(h.is_empty());

        Ok(())
    }

    #[derive(Debug, PartialEq)]
    struct Test {
        s: String,
    }

    impl Test {
        fn new(s: &str) -> Self {
            Self { s: s.to_string() }
        }
    }
    impl Drop for Test {
        fn drop(&mut self) {
            eprintln!("Dropping {}", self.s);
        }
    }

    impl KHashFunc for Test {
        fn hash(&self) -> u32 {
            hash_u8_slice(self.s.as_bytes())
        }
    }

    #[test]
    fn hash_int_string() -> Result<(), KHashError> {
        let mut h = KHashMap::init();
        assert_eq!(h.insert(42u32, Test::new("string1"))?, None);
        assert_eq!(h.insert(64, Test::new("string2"))?, None);
        assert_eq!(h.insert(1, Test::new("string3"))?, None);
        assert_eq!(h.insert(73, Test::new("string4"))?, None);
        assert_eq!(h.insert(1024, Test::new("string5"))?, None);
        assert_eq!(
            h.insert(64, Test::new("string6"))?,
            Some(Test::new("string2"))
        );

        assert_eq!(h.delete(&1), Some(Test::new("string3")));
        assert_eq!(h.insert(1, Test::new("string7"))?, None);

        for (k, v) in &mut h {
            eprintln!("{} {:?}", k, v)
        }

        Ok(())
    }

    #[test]
    fn hash_tstring() -> Result<(), KHashError> {
        let mut h = KHashMap::init();
        assert_eq!(h.insert(Test::new("key1"), 42)?, None);
        assert_eq!(h.insert(Test::new("key2"), 76)?, None);
        assert_eq!(h.insert(Test::new("key1"), 21)?, Some(42));

        assert_eq!(h.len(), 2);

        // Test into_keys iterator
        let k = h.into_keys().next().unwrap();
        assert_eq!(k, Test::new("key1"));
        Ok(())
    }

    #[test]
    fn hash_kstring() -> Result<(), KHashError> {
        let mut h = KHashMap::init();
        let ks = KString::from_str("key1").unwrap();
        assert_eq!(h.insert(ks, 42)?, None);
        Ok(())
    }

    #[test]
    fn hash_str() -> Result<(), KHashError> {
        let mut h = KHashMap::init();
        assert_eq!(h.insert("key1", 42)?, None);
        assert_eq!(h.insert("key2", 76)?, None);
        assert_eq!(h.insert("key1", 21)?, Some(42));

        // Test keys iterator
        assert_eq!(h.keys().next(), Some(&"key1"));
        Ok(())
    }

    #[test]
    fn hash_tstring2() -> Result<(), KHashError> {
        let mut h = KHashMap::init();
        assert_eq!(h.insert(Test::new("key1"), Test::new("val1"))?, None);
        assert_eq!(h.insert(Test::new("key2"), Test::new("val2"))?, None);
        assert_eq!(h.insert(Test::new("key3"), Test::new("val3"))?, None);
        assert_eq!(h.insert(Test::new("key4"), Test::new("val4"))?, None);
        assert_eq!(h.insert(Test::new("key5"), Test::new("val5"))?, None);

        assert_eq!(h.into_keys().nth(3).unwrap(), Test::new("key4"));
        Ok(())
    }

    #[test]
    fn hash_tstring3() -> Result<(), KHashError> {
        let mut h = KHashMap::init();
        assert_eq!(h.insert(Test::new("keyE"), Test::new("val1"))?, None);
        assert_eq!(h.insert(Test::new("keyD"), Test::new("val2"))?, None);
        assert_eq!(h.insert(Test::new("keyC"), Test::new("val3"))?, None);
        assert_eq!(h.insert(Test::new("keyB"), Test::new("val4"))?, None);
        assert_eq!(h.insert(Test::new("keyA"), Test::new("val5"))?, None);

        assert_eq!(h.into_values().nth(3).unwrap(), Test::new("val2"));
        Ok(())
    }

    #[test]
    fn hash_tstring4() -> Result<(), KHashError> {
        let mut h = KHashMap::init();
        assert_eq!(h.insert(Test::new("keyE"), Test::new("val1"))?, None);
        assert_eq!(h.insert(Test::new("keyD"), Test::new("val2"))?, None);
        assert_eq!(h.insert(Test::new("keyC"), Test::new("val3"))?, None);
        assert_eq!(h.insert(Test::new("keyB"), Test::new("val4"))?, None);
        assert_eq!(h.insert(Test::new("keyA"), Test::new("val5"))?, None);

        assert_eq!(
            h.drain().nth(3).unwrap(),
            (Test::new("keyD"), Test::new("val2"))
        );
        Ok(())
    }

    #[test]
    fn hash_tstring5() -> Result<(), KHashError> {
        let mut h = KHashMap::init();
        assert_eq!(h.insert(Test::new("keyE"), Test::new("val1"))?, None);
        assert_eq!(h.insert(Test::new("keyD"), Test::new("val2"))?, None);
        assert_eq!(h.insert(Test::new("keyC"), Test::new("val3"))?, None);
        assert_eq!(h.insert(Test::new("keyB"), Test::new("val4"))?, None);
        assert_eq!(h.insert(Test::new("keyA"), Test::new("val5"))?, None);

        assert_eq!(
            h.into_iter().nth(3).unwrap(),
            (Test::new("keyD"), Test::new("val2"))
        );
        Ok(())
    }
}
