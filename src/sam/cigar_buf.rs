use super::{
    cigar::{self, cigar_len, Cigar, CigarElem},
    cigar_error::{CigarError, CigarTrimError},
    cigar_validate::valid_elem_slice,
};

use crate::sam::cigar::CigarOp;
use std::{
    fmt::{self, Formatter},
    ops::Deref,
    str::FromStr,
};

#[derive(Default, Debug, Clone)]
pub struct CigarBuf {
    vec: Vec<CigarElem>,
}

impl Deref for CigarBuf {
    type Target = Cigar;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { cigar::from_elems_unchecked(&self.vec) }
    }
}

impl AsRef<Cigar> for CigarBuf {
    #[inline]
    fn as_ref(&self) -> &Cigar {
        self
    }
}

impl CigarBuf {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(sz: usize) -> Self {
        let vec = Vec::with_capacity(sz);
        Self { vec }
    }

    /// # Safety
    ///
    /// The caller must assure that the resulting Cigar is valid
    #[inline]
    pub unsafe fn push_unchecked(&mut self, e: CigarElem) {
        self.vec.push(e)
    }

    #[inline]
    pub fn push_checked(&mut self, e: CigarElem) -> Result<(), CigarError> {
        self.vec.push(e);
        valid_elem_slice(self).map_err(|e| {
            self.vec.pop();
            e
        })
    }

    #[inline]
    pub fn clear(&mut self) {
        self.vec.clear()
    }

    #[inline]
    pub fn pop(&mut self) -> Option<CigarElem> {
        self.vec.pop()
    }

    #[inline]
    pub fn last(&self) -> Option<&CigarElem> {
        self.vec.last()
    }

    #[inline]
    pub fn first(&self) -> Option<&CigarElem> {
        self.vec.first()
    }

    /// # Safety
    ///
    /// The caller must assure that the resulting Cigar is valid
    #[inline]
    pub unsafe fn from_vec_unchecked(vec: Vec<CigarElem>) -> Self {
        Self { vec }
    }

    #[inline]
    pub fn from_vec(vec: Vec<CigarElem>) -> Result<Self, CigarError> {
        valid_elem_slice(&vec).map(|_| Self { vec })
    }

    #[inline]
    pub fn from_cigar(c: &Cigar) -> Self {
        let vec = c.to_vec();
        Self { vec }
    }

    // Adjust cigar so that alignment starts x bases later w.r.t the reference by adding/converting
    // Cigar ops to OVERLAP
    pub fn trim_start(&mut self, x: u32) -> Result<(), CigarError> {
        if let Ok(vec) = trim_cigar_vec(self.iter().copied(), x) {
            self.vec = vec;
            Ok(())
        } else {
            Err(CigarError::CigarTooShortForTrim)
        }
    }

    // Adjust cigar so that alignment ends x bases earlier w.r.t the reference
    pub fn trim_end(&mut self, x: u32) -> Result<(), CigarError> {
        if let Ok(mut v) = trim_cigar_vec(self.iter().copied().rev(), x) {
            let vec: Vec<_> = v.drain(..).rev().collect();
            self.vec = vec;
            Ok(())
        } else {
            Err(CigarError::CigarTooShortForTrim)
        }
    }
}

impl fmt::Display for CigarBuf {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl FromStr for CigarBuf {
    type Err = CigarError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut cb = Self::new();
        let mut s = s;
        while !s.is_empty() {
            let (elem, s_new) = CigarElem::parse(s)?;
            s = s_new;
            unsafe { cb.push_unchecked(elem) }
        }
        valid_elem_slice(&cb).map(|_| cb)
    }
}

fn trim_cigar_vec<I: Iterator<Item = CigarElem>>(
    it: I,
    x: u32,
) -> Result<Vec<CigarElem>, CigarTrimError> {
    assert!(x < (1 << 24), "Trim distance too large");
    let mut ct = 0;
    let mut v = Vec::new();
    let mut ql = 0;
    let mut rl = 0;
    for elem in it {
        let l = elem.op_len();
        if elem.consumes_query() {
            ql += l
        }
        let con_ref = elem.consumes_reference();
        if con_ref {
            rl += l
        }
        if ct >= x || !con_ref {
            v.push(elem)
        } else {
            if (elem.op_type() & 1) != 0 {
                if ct + l <= x {
                    v.push(unsafe { CigarElem::from_parts_unchecked(CigarOp::Overlap, l) });
                } else if x >= ct {
                    v.push(unsafe { CigarElem::from_parts_unchecked(CigarOp::Overlap, x - ct) });
                    let x1 = ct + l - x;
                    assert!(x1 < (1 << 24));
                    v.push(unsafe { CigarElem::from_parts_unchecked(elem.op(), x1) });
                } else {
                    v.push(elem)
                }
            } else {
                v.push(elem)
            }
            ct += l;
        }
    }
    assert_eq!(
        cigar_len(&v, |c| c.consumes_query()),
        ql,
        "Mismatch in query length after trim"
    );
    let rl1 = if rl > x { rl - x } else { 0 };
    assert_eq!(
        cigar_len(&v, |c| c.consumes_reference()),
        rl1,
        "Mismatch in expected reference length after trim"
    );
    if rl >= x {
        Ok(v)
    } else {
        Err(CigarTrimError::CigarTooShort(v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CigarError;
    #[test]
    fn construction() {
        let mut cb = "5S80M2S6H"
            .parse::<CigarBuf>()
            .expect("Error parsing Cigar");
        assert_eq!(format!("{cb}"), "5S80M2S6H");
        assert_eq!(
            cb.push_checked("3S".parse::<CigarElem>().unwrap()),
            Err(CigarError::InteriorHardClip)
        );
        assert_eq!(
            cb.push_checked("32H".parse::<CigarElem>().unwrap()),
            Err(CigarError::MultipleAdjacentHardClips)
        );
        cb.clear();
        assert_eq!(format!("{cb}"), "*");
        cb.push_checked("1S".parse::<CigarElem>().unwrap()).unwrap();
        cb.push_checked("32M".parse::<CigarElem>().unwrap())
            .unwrap();
        cb.push_checked("5S".parse::<CigarElem>().unwrap()).unwrap();
        assert_eq!(
            cb.push_checked("1M".parse::<CigarElem>().unwrap()),
            Err(CigarError::InteriorSoftClip)
        );
        cb.push_checked("1H".parse::<CigarElem>().unwrap()).unwrap();
        assert_eq!(format!("{cb}"), "1S32M5S1H");
    }

    #[test]
    fn lengths() {
        let cb = "2H5S80M2S6H"
            .parse::<CigarBuf>()
            .expect("Error parsing Cigar");

        assert_eq!(cb.query_len(), 87);
        assert_eq!(cb.query_len_including_hard_clips(), 95);
        assert_eq!(cb.reference_len(), 80);

        let cb = "1S80M3I5M1D10M"
            .parse::<CigarBuf>()
            .expect("Error parsing Cigar");

        assert_eq!(cb.query_len(), 99);
        assert_eq!(cb.reference_len(), 96);
    }

    #[test]
    fn trim() {
        let cb = "5S80M1D5M2I7M2S"
            .parse::<CigarBuf>()
            .expect("Error parsing Cigar");

        let mut cb1 = cb.clone();
        cb1.trim_start(6).unwrap();
        assert_eq!(format!("{cb1}"), "5S6O74M1D5M2I7M2S");
        cb1 = cb.clone();
        cb1.trim_end(8).unwrap();
        assert_eq!(format!("{cb1}"), "5S80M1D4M1O2I7O2S");
    }
}
