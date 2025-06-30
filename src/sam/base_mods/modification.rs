use std::fmt;

use super::{CanonicalBase, ModifiedBase};
use crate::BaseModsError;

/// The processed modification code where the base modification code and ChEBI codes are put
/// together if possible, and the correspondence between the canonical base and the modification
/// is checked if known (i.e., ModifiedBase::ChEBI(27551) and ModifiedBase::BaseCode(b'm') will
/// both be translated to the same type of Modification, and a check will be made that the
/// canonical base is C as this is the code for 5mC.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Modification {
    // We pack everything into a u64 to have a more efficient storage
    //
    // bits 0..7 - MF flags (see below)
    // bits 8..39 - ChEBI code if known
    // bits 40..47 - base_mod_code if known
    // bits 48..55 - CanonicalBase
    // bits 56..63 - mod likelihood score (0-255)
    inner: u64,
}

pub const MF_REVERSE_STRAND: u8 = 1;
pub const MF_MOD_CODE_PRESENT: u8 = 2;
pub const MF_CHEBI_PRESENT: u8 = 4;
pub const MF_ML_PRESENT: u8 = 8;
pub const MF_ML_EXPLICIT: u8 = 16;

impl fmt::Display for Modification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let strand = if self.is_reversed() { '-' } else { '+' };
        if f.alternate() {
            if let Some(b) = self.base_mod_code() {
                if let Some(s) = match b {
                    b'm' => Some("5mC"),
                    b'h' => Some("5hmC"),
                    b'f' => Some("5fC"),
                    b'c' => Some("5caC"),
                    b'C' => Some("?C"),
                    b'g' => Some("5hmU"),
                    b'e' => Some("5fU"),
                    b'b' => Some("5caU"),
                    b'T' => Some("?T"),
                    b'U' => Some("?U"),
                    b'a' => Some("6mA"),
                    b'A' => Some("?A"),
                    b'o' => Some("8oxoG"),
                    b'G' => Some("?G"),
                    b'n' => Some("Xao"),
                    b'N' => Some("?N"),
                    _ => None,
                } {
                    write!(f, "{s}")?
                } else {
                    write!(f, "[{}]{}", b as char, self.canonical_base())?
                }
            } else {
                let x = self.chebi_code().expect("Missing ChEBI code");
                write!(f, "({}){}", x, self.canonical_base())?
            }
            write!(f, "{strand}")
        } else {
            write!(f, "{}{}", self.canonical_base(), strand)?;
            if let Some(b) = self.base_mod_code() {
                write!(f, "{}", b as char)
            } else {
                write!(f, "{}", self.chebi_code().expect("Missing ChEBI code"))
            }
        }
    }
}

impl fmt::Debug for Modification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Modification {{ ")?;
        write!(f, "canonical base: {:?}", self.canonical_base())?;
        write!(f, ", flags: 0x{:02x}", self.flags())?;
        write!(f, ", base_mod_code: {:?}", self.base_mod_code())?;
        write!(f, ", chebi_code: {:?}", self.chebi_code())?;
        write!(f, ", ml_value: {:?}", self.ml_value())?;
        write!(f, " }}")
    }
}

impl Modification {
    /// Generate [Modification] from a [CanonicalBase] and a [ModifiedBase], with checks made
    /// that the two match for known modifications
    pub fn new(
        canonical_base: CanonicalBase,
        modified_base: ModifiedBase,
        reverse_strand: bool,
    ) -> Result<Self, BaseModsError> {
        match modified_base {
            ModifiedBase::BaseCode(b) => {
                Self::_new(canonical_base, Some(b), base_to_chebi(b), reverse_strand)
            }
            ModifiedBase::ChEBI(x) => {
                Self::_new(canonical_base, chebi_to_base(x), Some(x), reverse_strand)
            }
        }
    }

    #[inline]
    pub fn flags(&self) -> u8 {
        (self.inner & 0xff) as u8
    }

    pub fn base_mod_code(&self) -> Option<u8> {
        if self.flags() & MF_MOD_CODE_PRESENT == 0 {
            None
        } else {
            Some(((self.inner >> 40) & 0xff) as u8)
        }
    }

    pub fn chebi_code(&self) -> Option<u32> {
        if self.flags() & MF_CHEBI_PRESENT == 0 {
            None
        } else {
            Some(((self.inner >> 8) & 0xffffffff) as u32)
        }
    }

    pub fn ml_value(&self) -> Option<u8> {
        if self.flags() & MF_ML_PRESENT == 0 {
            None
        } else {
            Some((self.inner >> 56) as u8)
        }
    }

    pub fn set_ml_value(&mut self, x: u8) {
        self.inner = (self.inner & 0x00ffffffffffffff) | (x as u64) << 56 | (MF_ML_EXPLICIT | MF_ML_PRESENT) as u64;
    }
    
    pub fn set_implicit_ml_value(&mut self) {
        self.inner = (self.inner & 0x00ffffffffffffdf) | MF_ML_PRESENT as u64;
    }
    
    pub fn canonical_base(&self) -> CanonicalBase {
        unsafe { CanonicalBase::from_raw_unchecked(((self.inner >> 48) & 0xff) as u8) }
    }

    fn _new(
        canonical_base: CanonicalBase,
        base_mod_code: Option<u8>,
        chebi_code: Option<u32>,
        reverse_strand: bool,
    ) -> Result<Self, BaseModsError> {
        assert!(base_mod_code.is_some() || chebi_code.is_some());

        if let Some(b) = base_mod_code {
            let c = {
                let cb = match b {
                    b'h' | b'm' | b'f' | b'c' | b'C' => CanonicalBase::C,
                    b'g' | b'e' | b'b' | b'T' => CanonicalBase::T,
                    b'U' => CanonicalBase::U,
                    b'a' | b'A' => CanonicalBase::A,
                    b'o' | b'G' => CanonicalBase::G,
                    b'n' | b'N' => CanonicalBase::N,
                    _ => return Err(BaseModsError::UnknownModCode(b as char)),
                };
                if reverse_strand { cb.complement() } else { cb }
            };

            if c != canonical_base {
                return Err(BaseModsError::ModifierMismatch(
                    canonical_base,
                    if reverse_strand { '-' } else { '+' },
                    b as char,
                ));
            }
        }
        let flags = if reverse_strand { MF_REVERSE_STRAND } else { 0 };

        let (flags, base_mod) = if let Some(x) = base_mod_code {
            (flags | MF_MOD_CODE_PRESENT, x)
        } else {
            (flags, 0)
        };

        let (flags, chebi) = if let Some(x) = chebi_code {
            (flags | MF_CHEBI_PRESENT, x)
        } else {
            (flags, 0)
        };

        let inner = flags as u64
            | (chebi as u64) << 8
            | (base_mod as u64) << 40
            | (canonical_base.to_raw() as u64) << 48;

        Ok(Self { inner })
    }

    pub fn is_reversed(&self) -> bool {
        self.flags() & MF_REVERSE_STRAND != 0
    }
    
    pub fn has_explicit_ml(&self) -> bool {
        self.flags() & (MF_ML_PRESENT | MF_ML_EXPLICIT) == MF_ML_PRESENT | MF_ML_EXPLICIT
    }

    /// Parse modifications in the format found in the MM tag.  The modifications found
    /// are added to the mods vector.
    pub(super) fn from_u8_slice(v: &[u8], mods: &mut Vec<Self>) -> Result<usize, BaseModsError> {
        if v.len() < 3 {
            return Err(BaseModsError::ShortInput);
        }
        let canonical_base = CanonicalBase::from_u8(v[0])?;
        let reverse_strand = is_reverse_strand(v[1])?;
        let ix = if v[2].is_ascii_alphabetic() {
            let mut v_end = v.len();
            for (i, b) in v[2..].iter().enumerate() {
                if b.is_ascii_alphabetic() {
                    mods.push(Modification::new(
                        canonical_base,
                        ModifiedBase::BaseCode(*b),
                        reverse_strand,
                    )?)
                } else {
                    v_end = 2 + i;
                    break;
                }
            }
            v_end
        } else if v[2].is_ascii_digit() {
            let (mb, v_end) = ModifiedBase::parse_ch_ebi(&v[2..])?;
            mods.push(Modification::new(canonical_base, mb, reverse_strand)?);
            v_end + 2
        } else {
            return Err(BaseModsError::BadModifierBase(v[2]));
        };
        Ok(ix)
    }
}

fn chebi_to_base(x: u32) -> Option<u8> {
    match x {
        27551 => Some(b'm'),
        76792 => Some(b'h'),
        76794 => Some(b'f'),
        76793 => Some(b'c'),
        16964 => Some(b'g'),
        80961 => Some(b'e'),
        17477 => Some(b'b'),
        28871 => Some(b'a'),
        44605 => Some(b'o'),
        18107 => Some(b'n'),
        _ => None,
    }
}

fn base_to_chebi(b: u8) -> Option<u32> {
    match b {
        b'm' => Some(27551),
        b'h' => Some(76792),
        b'f' => Some(76794),
        b'c' => Some(76793),
        b'g' => Some(16964),
        b'e' => Some(80961),
        b'b' => Some(17477),
        b'a' => Some(28871),
        b'o' => Some(44605),
        b'n' => Some(18107),
        _ => None,
    }
}

/// Returns true if strand is '-' (reverse)
fn is_reverse_strand(c: u8) -> Result<bool, BaseModsError> {
    match c {
        b'+' => Ok(false),
        b'-' => Ok(true),
        _ => Err(BaseModsError::MalformedMMTag),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_modifications() -> Result<(), BaseModsError> {
        let mut mods = Vec::new();
        let ix = Modification::from_u8_slice("C+mh".as_bytes(), &mut mods)?;
        assert_eq!(ix, 4);
        let s = format!("{}", mods[1]);
        assert_eq!(s.as_str(), "C+h");
        mods.clear();
        let _ = Modification::from_u8_slice("G-m".as_bytes(), &mut mods)?;
        let s = format!("{}", mods[0]);
        assert_eq!(s.as_str(), "G-m");
        Ok(())
    }
}
