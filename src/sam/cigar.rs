use std::{
    convert::{AsMut, AsRef},
    fmt::{self, Formatter},
    mem::transmute,
    ops::Deref,
    str::FromStr,
};

use super::{cigar_buf::CigarBuf, cigar_error::CigarError, cigar_validate::valid_elem_slice};

#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Copy, Clone)]
pub enum CigarOp {
    Match,
    Ins,
    Del,
    RefSkip,
    SoftClip,
    HardClip,
    Pad,
    Equal,
    Diff,
    Back,
    Overlap,
    Invalid1,
    Invalid2,
    Invalid3,
    Invalid4,
    Invalid5,
}

const CIGAR_DISPLAY: [char; 16] = [
    'M', 'I', 'D', 'N', 'S', 'H', 'P', '=', 'X', 'B', 'O', '?', '?', '?', '?', '?',
];

impl fmt::Display for CigarOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", CIGAR_DISPLAY[*self as usize])
    }
}

impl FromStr for CigarOp {
    type Err = CigarError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "M" => Ok(CigarOp::Match),
            "I" => Ok(CigarOp::Ins),
            "D" => Ok(CigarOp::Del),
            "N" => Ok(CigarOp::RefSkip),
            "S" => Ok(CigarOp::SoftClip),
            "H" => Ok(CigarOp::HardClip),
            "P" => Ok(CigarOp::Pad),
            "=" => Ok(CigarOp::Equal),
            "X" => Ok(CigarOp::Diff),
            "B" => Ok(CigarOp::Back),
            "O" => Ok(CigarOp::Overlap),
            _ => Err(CigarError::UnknownOperator),
        }
    }
}

impl CigarOp {
    pub fn is_valid(&self) -> bool {
        *self < Self::Invalid1
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct CigarElem(u32);

const CIGAR_TYPE: u32 = 0x13C1A7;
const CIGAR_TYPE1: u32 = 0x13C5A7;

impl CigarElem {
    #[inline]
    pub fn op_len(&self) -> u32 {
        self.0 >> 4
    }

    #[inline]
    pub fn op(&self) -> CigarOp {
        unsafe { transmute((self.0 & 15) as u8) }
    }

    #[inline]
    pub fn op_pair(&self) -> (CigarOp, u32) {
        (self.op(), self.op_len())
    }

    // This magic comes from htslib/sam.h
    // If bit 0 is set in op_type then the op consumes the query, and
    // if bit 1 is set then the op consumes the reference
    #[inline]
    pub fn op_type(&self) -> u32 {
        (CIGAR_TYPE >> ((self.0 & 15) << 1)) & 3
    }

    #[inline]
    pub fn consumes_reference(&self) -> bool {
        (self.op_type() & 2) != 0
    }

    #[inline]
    pub fn consumes_query(&self) -> bool {
        (self.op_type() & 1) != 0
    }

    #[inline]
    pub fn consumes_query_including_hard_clips(&self) -> bool {
        (self.op_type1() & 1) != 0
    }

    #[inline]
    // Similar to above, but we also count Hard clips the same as Soft clips
    pub fn op_type1(&self) -> u32 {
        (CIGAR_TYPE1 >> ((self.0 & 15) << 1)) & 3
    }

    pub(super) fn parse(s: &str) -> Result<(Self, &str), CigarError> {
        if let Some(i) = s.find(|c| !char::is_digit(c, 10)) {
            if i > 0 {
                let (s1, s2) = s.split_at(i);
                let (s2, s3) = s2.split_at(1);
                match (s1.parse::<u32>(), s2.parse::<CigarOp>()) {
                    (Ok(l), Ok(op)) => {
                        if l < (1 << 28) {
                            Ok((unsafe { Self::from_parts_unchecked(op, l) }, s3))
                        } else {
                            Err(CigarError::BadLength)
                        }
                    }
                    (Err(_), _) => Err(CigarError::BadLength),
                    (_, Err(e)) => Err(e),
                }
            } else {
                Err(CigarError::MissingLength)
            }
        } else {
            Err(CigarError::MissingOperator)
        }
    }
    /// # Safety
    ///
    /// Caller must assure that op is valid and l < (1 << 28)
    pub(super) unsafe fn from_parts_unchecked(op: CigarOp, l: u32) -> Self {
        Self((l << 4) | (op as u32))
    }

    pub fn from_parts(op: CigarOp, len: u32) -> Result<Self, CigarError> {
        if op.is_valid() {
            if len < (1 << 28) {
                Ok(unsafe { Self::from_parts_unchecked(op, len) })
            } else {
                Err(CigarError::BadLength)
            }
        } else {
            Err(CigarError::UnknownOperator)
        }
    }
}

impl fmt::Display for CigarElem {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.op_len(), self.op())
    }
}

impl FromStr for CigarElem {
    type Err = CigarError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match Self::parse(s) {
            Ok((elem, "")) => Ok(elem),
            Ok((_, _)) => Err(CigarError::TrailingGarbage),
            Err(e) => Err(e),
        }
    }
}

#[derive(Eq, PartialEq)]
pub struct Cigar([CigarElem]);

impl Cigar {
    #[inline]
    pub fn as_elems(&self) -> &[CigarElem] {
        &self.0
    }
    #[inline]
    pub fn as_elems_mut(&mut self) -> &mut [CigarElem] {
        &mut self.0
    }
}

/// Convert from CigarElem slice to &Cigar
///
/// # Safety
///   The caller must guarantee that the slice v forms a valid Cigar
#[inline]
pub const unsafe fn from_elems_unchecked(v: &[CigarElem]) -> &Cigar {
    unsafe { transmute(v) }
}

/// Convert from mutable CigarElem slice to &mut Cigar
///
/// # Safety
///   The caller must guarantee that the slice v forms a valid Cigar
#[inline]
pub unsafe fn from_elems_unchecked_mut(v: &mut [CigarElem]) -> &mut Cigar {
    unsafe { &mut *(v as *mut [CigarElem] as *mut Cigar) }
}

#[inline]
pub fn from_elems(v: &[CigarElem]) -> Result<&Cigar, CigarError> {
    valid_elem_slice(v).map(|_| unsafe { from_elems_unchecked(v) })
}

#[inline]
pub fn from_elems_mut(v: &mut [CigarElem]) -> Result<&mut Cigar, CigarError> {
    valid_elem_slice(v).map(|_| unsafe { from_elems_unchecked_mut(v) })
}

impl AsRef<Cigar> for Cigar {
    #[inline]
    fn as_ref(&self) -> &Cigar {
        self
    }
}

impl AsRef<[CigarElem]> for Cigar {
    #[inline]
    fn as_ref(&self) -> &[CigarElem] {
        self.as_elems()
    }
}

impl AsMut<Cigar> for Cigar {
    #[inline]
    fn as_mut(&mut self) -> &mut Cigar {
        self
    }
}

impl Deref for Cigar {
    type Target = [CigarElem];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_elems()
    }
}

impl fmt::Display for Cigar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_empty() {
            write!(f, "*")
        } else {
            for e in self.iter() {
                write!(f, "{e}")?
            }
            Ok(())
        }
    }
}

impl Cigar {
    /// Returns length on query of cigar
    #[inline]
    pub fn query_len(&self) -> u32 {
        cigar_len(self, |c| c.consumes_query())
    }

    /// Returns length on query of cigar including hard clips
    #[inline]
    pub fn query_len_including_hard_clips(&self) -> u32 {
        cigar_len(self, |c| c.consumes_query_including_hard_clips())
    }

    /// Returns length on reference of cigar
    #[inline]
    pub fn reference_len(&self) -> u32 {
        cigar_len(self, |c| c.consumes_reference())
    }
    #[inline]
    pub fn to_cigar_buf(&self) -> CigarBuf {
        CigarBuf::from_cigar(self)
    }
    #[inline]
    pub fn to_owned(&self) -> CigarBuf {
        self.to_cigar_buf()
    }
}

pub(super) fn cigar_len<F>(v: &[CigarElem], f: F) -> u32
where
    F: FnMut(&&CigarElem) -> bool,
{
    v.iter().filter(f).fold(0, |mut l, c| {
        l += c.op_len();
        l
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CigarError;
    #[test]
    fn construction() {
        let el = "213M".parse::<CigarElem>().expect("Error parsing element");
        assert_eq!(format!("{el}"), "213M");
        assert_eq!(
            "2S1H".parse::<CigarElem>(),
            Err(CigarError::TrailingGarbage)
        );
    }
}
