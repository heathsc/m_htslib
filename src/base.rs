use std::fmt;

use crate::sam::SeqComplement;

/// A base represents the IUPAC ambiguity codes
/// There are 16 possible codes, so Base can not be more than 15
///
/// 0 -> No base
/// 1 -> A
/// 2 -> C
/// 3 -> M (A | C)
/// 4 -> G
/// 5 -> R (A | G)
/// 6 -> S (C | G)
/// 7 -> V (A | C | G)
/// 8 -> T
/// 9 -> W (A | T)
/// 10 -> Y (C | T)
/// 11 -> H (A | C | T)
/// 12 -> K (G | T)
/// 13 -> D (A | G | T)
/// 14 -> B (C | G | T)
/// 15 -> N (A | C | G | T)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Base(u8);

impl Base {
    #[inline]
    pub fn new(x: u8) -> Self {
        Self(x & 0xf)
    }

    #[inline]
    pub fn from_u8(c: u8) -> Self {
        Self(SEQ_NT16_TABLE[c as usize])
    }

    #[inline]
    pub(crate) fn combine(&self, other: &Self) -> u8 {
        (self.0 << 4) | other.0
    }

    #[inline]
    pub fn as_n(&self) -> u8 {
        self.0
    }

    #[inline]
    pub fn as_char(&self) -> char {
        BASE_TABLE[self.0 as usize] as char
    }

    #[inline]
    pub fn complement(&self) -> Self {
        Self(self.0.reverse_bits() >> 4)
    }
}

impl SeqComplement for Base {
    fn get_complement(&self) -> Self {
        self.complement()
    }
}

impl fmt::Display for Base {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_char())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BaseQual {
    base: Base,
    qual: u8,
}

impl BaseQual {
    #[inline]
    pub fn new(base: Base, qual: u8) -> Self {
        Self { base, qual }
    }
    
    #[inline]
    pub fn base(&self) -> Base {
        self.base
    }
    
    #[inline]
    pub fn qual(&self) -> u8 {
        self.qual
    }
    
    #[inline]
    pub fn base_qual(&self) -> (Base, u8) {
        (self.base, self.qual)
    }
}

impl SeqComplement for BaseQual {
    fn get_complement(&self) -> Self {
        Self { base: self.base.complement(), qual: self.qual }
    }
}

const BASE_TABLE: &[u8; 16] = b"-ACMGRSVTWYHKDBN";

const SEQ_NT16_TABLE: [u8; 256] = [
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    1, 2, 4, 8, 15, 15, 15, 15, 15, 15, 15, 15, 15, 0, 15, 15, 15, 1, 14, 2, 13, 15, 15, 4, 11, 15,
    15, 12, 15, 3, 15, 15, 15, 15, 5, 6, 8, 15, 7, 9, 15, 10, 15, 15, 15, 15, 15, 15, 15, 1, 14, 2,
    13, 15, 15, 4, 11, 15, 15, 12, 15, 3, 15, 15, 15, 15, 5, 6, 8, 15, 7, 9, 15, 10, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
];
