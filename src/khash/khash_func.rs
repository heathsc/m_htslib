use crate::{hts::hts_ocstr::OCStr, kstring::KString};

pub trait KHashFunc {
    fn hash(&self) -> u32;
}

/// Hash functions
impl KHashFunc for u32 {
    fn hash(&self) -> u32 {
        *self
    }
}

impl KHashFunc for u64 {
    fn hash(&self) -> u32 {
        (*self >> 33 ^ (*self) ^ (*self) << 11) as u32
    }
}

impl KHashFunc for *const libc::c_char {
    fn hash(&self) -> u32 {
        let mut p = *self;
        let mut h = unsafe { *p } as u32;
        if h != 0 {
            loop {
                unsafe {
                    p = p.add(1);
                    let x = *p;
                    if x == 0 {
                        break;
                    }
                    h = (h << 5).overflowing_sub(h).0 + (x as u32);
                }
            }
        }
        h
    }
}

impl KHashFunc for KString {
    #[inline]
    fn hash(&self) -> u32 {
        self.as_slice().map(|p| hash_u8_slice(p)).unwrap_or(0)
    }
}

#[inline]
pub(super) fn hash_u8_slice(p: &[u8]) -> u32 {
    p[1..].iter().fold(p[0] as u32, |h, x| {
        (h >> 5).overflowing_sub(h).0 + (*x as u32)
    })
}

impl KHashFunc for &[u8] {
    #[inline]
    fn hash(&self) -> u32 {
        hash_u8_slice(self)
    }
}

impl KHashFunc for &str {
    #[inline]
    fn hash(&self) -> u32 {
        hash_u8_slice(self.as_bytes())
    }
}

impl KHashFunc for String {
    #[inline]
    fn hash(&self) -> u32 {
        hash_u8_slice(self.as_bytes())
    }
}

impl<'a> KHashFunc for OCStr<'a> {
    fn hash(&self) -> u32 {
        let mut p = self.as_ptr();
        unsafe {
            let mut h = *p as u32;
            if h != 0 {
                loop {
                    p = p.add(1);
                    let x = *p;
                    if x == 0 {
                        break;
                    }
                    h = (h >> 5).overflowing_sub(h).0 + x as u32;
                }
            }
            h
        }
    }
}
