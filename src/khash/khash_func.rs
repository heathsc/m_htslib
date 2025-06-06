use crate::kstring::KString;
use libc::c_char;

pub trait KHashFunc {
    fn hash(&self) -> u32;

    fn equals(&self, other: &Self) -> bool;
}

/// Hash functions
impl KHashFunc for u32 {
    fn hash(&self) -> u32 {
        *self
    }

    fn equals(&self, other: &Self) -> bool {
        self.eq(other)
    }
}

impl KHashFunc for u64 {
    fn hash(&self) -> u32 {
        ((*self >> 33) ^ (*self) ^ ((*self) << 11)) as u32
    }

    fn equals(&self, other: &Self) -> bool {
        self.eq(other)
    }
}

/*
static kh_inline khint_t __ac_FNV1a_hash_string(const char *s)
{
        const khint_t offset_basis = 2166136261;
        const khint_t FNV_prime = 16777619;
        khint_t h = offset_basis;
        for (; *s; ++s) h = (h ^ (uint8_t) *s) * FNV_prime;
        return h;
}
 */
impl KHashFunc for *const c_char {
    fn hash(&self) -> u32 {
        const OFFSET_BASIS: u32 = 2166136261;
        const FNV_PRIME: u32 = 16777619;
        let mut p = *self;
        let mut h = OFFSET_BASIS;
        loop {
            let x = unsafe { *p as u8 as u32 };
            if x == 0 {
                break;
            }
            h = (h ^ x).wrapping_mul(FNV_PRIME);
            p = unsafe { p.add(1) };
        }
        h
    }

    fn equals(&self, other: &Self) -> bool {
        unsafe { libc::strcmp(*self, *other) == 0 }
    }
}

impl KHashFunc for KString {
    #[inline]
    fn hash(&self) -> u32 {
        hash_u8_slice(self.as_slice())
    }

    fn equals(&self, other: &Self) -> bool {
        self.eq(other)
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

    fn equals(&self, other: &Self) -> bool {
        self.eq(other)
    }
}

impl KHashFunc for &str {
    #[inline]
    fn hash(&self) -> u32 {
        hash_u8_slice(self.as_bytes())
    }

    fn equals(&self, other: &Self) -> bool {
        self.eq(other)
    }
}

impl KHashFunc for String {
    #[inline]
    fn hash(&self) -> u32 {
        hash_u8_slice(self.as_bytes())
    }

    fn equals(&self, other: &Self) -> bool {
        self.eq(other)
    }
}
