use std::fmt;

use crate::{int_utils::parse_uint, BaseModsError};

/// Representation of the original (unmodified) base.
/// The u8 representations are to match the sequence base codes from htslib.  Note that U actually
/// should have the representation 8 (like T), but enum codes of course have to be distinct so we
/// set U equal to 0 and then translate in CanonicalBase::to_u8().
#[derive(Default, Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum CanonicalBase {
    U = 0,
    A = 1,
    C = 2,
    G = 4,
    T = 8,
    #[default]
    N = 15,
}

const CBASE_CHAR: &str = "UAC?G???T??????N";

impl fmt::Display for CanonicalBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", CBASE_CHAR.as_bytes()[*self as usize] as char)
    }
}

impl CanonicalBase {
    pub fn from_u8(b: u8) -> Result<Self, BaseModsError> {
        match b {
            b'A' => Ok(Self::A),
            b'C' => Ok(Self::C),
            b'G' => Ok(Self::G),
            b'T' => Ok(Self::T),
            b'U' => Ok(Self::U),
            b'N' => Ok(Self::N),
            _ => Err(BaseModsError::IllegalCanonicalBase(b)),
        }
    }

    /// Creates a CanonicalBase directly from the underlying u8
    ///
    /// # Safety
    ///
    /// This is unsafe if a u8 is used that does not correspond to a canonical base (i.e., 9).
    /// It is instead recommended to use [Self::from_raw] that ensures that only valid values are used
    #[inline]
    pub unsafe fn from_raw_unchecked(x: u8) -> Self {
        unsafe { std::mem::transmute::<u8, Self>(x) }
    }
    
    const VALID_REPR: [bool; 16] = [true, true, true, false, true, false, false, false, true, false, false, false, false, false, false, true];
    
    pub fn from_raw(x: u8) -> Result<Self, BaseModsError> {
        if Self::VALID_REPR.get(x as usize).copied().unwrap_or(false) {
            Ok(unsafe { Self::from_raw_unchecked(x)} )
        } else {
            Err(BaseModsError::IllegalCanonicalBaseRepr(x))
        }
    }
    
    #[inline]
    pub fn to_raw(&self) -> u8 {
        *self as u8
    }
    
    /// This translation is to match with the sequence base codes from htslib. Note that we map
    /// U to T for the purpose of matching to the sequence
    pub fn as_u8(&self) -> u8 {
        match self {
            Self::U => 8,
            x => *x as u8,
        }
    }

    pub fn complement(&self) -> Self {
        match self {
            Self::A => Self::T,
            Self::C => Self::G,
            Self::G => Self::C,
            Self::T => Self::A,
            Self::U => Self::A,
            Self::N => Self::N,
        }
    }
}

/// The raw modification code as read from the MM tag.  This can either be a single character base
/// code (like m, h etc.) or a ChEBI numeric code
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ModifiedBase {
    BaseCode(u8),
    ChEBI(u32),
}

impl ModifiedBase {
    pub(super) fn parse_ch_ebi(v: &[u8]) -> Result<(Self, usize), BaseModsError> {
        let (v1, i) = if let Some((ix, _)) = v.iter().enumerate().find(|(_, c)| !c.is_ascii_digit())
        {
            assert!(ix > 0);
            (&v[0..ix], ix)
        } else {
            (v, v.len())
        };
        let (chebi, _) = parse_uint::<u32>(v1, u32::MAX)?;
            // .with_context(|| "Malformed MM tag - error parsing ChEBI value")?;
        Ok((Self::ChEBI(chebi), i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_canonical()-> Result<(), BaseModsError> {
        let cb = CanonicalBase::from_u8(b'C')?;
        let x = cb.to_raw();
        assert_eq!(x, 2);
        let cb1 = CanonicalBase::from_raw(x)?;
        assert_eq!(cb, cb1);
        Ok(())
    }
}