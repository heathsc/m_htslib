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

    /// This translation is to match with the sequence base codes from htslib.  Note that we map
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
