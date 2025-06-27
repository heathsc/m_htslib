use std::fmt;

use crate::BaseModsError;
use super::{CanonicalBase, ModifiedBase};

/// The processed modification code where the base modification code and ChEBI codes are put
/// together if possible, and the correspondence between the canonical base and the modification
/// is checked if known (i.e., ModifiedBase::ChEBI(27551) and ModifiedBase::BaseCode(b'm') will
/// both be translated to the same type of Modification, and a check will be made that the
/// canonical base is C as this is the code for 5mC.
#[derive(Default, Copy, Clone, Debug, Eq, PartialEq)]
pub struct Modification {
    canonical_base: CanonicalBase,
    reverse_strand: bool,
    base_mod_code: Option<u8>,
    chebi_code: Option<u32>,
}

impl fmt::Display for Modification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let strand = if self.reverse_strand { '-' } else { '+' };
        if f.alternate() {
            if let Some(b) = self.base_mod_code {
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
                    write!(f, "{}", s)?
                } else {
                    write!(f, "[{}]{}", b as char, self.canonical_base)?
                }
            } else {
                let x = self.chebi_code.expect("Missing ChEBI code");
                write!(f, "({}){}", x, self.canonical_base)?
            }
            write!(f, "{}", strand)
        } else {
            write!(f, "{}{}", self.canonical_base, strand)?;
            if let Some(b) = self.base_mod_code {
                write!(f, "{}", b as char)
            } else {
                write!(f, "{}", self.chebi_code.expect("Missing ChEBI code"))
            }
        }
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

    pub fn base_mod_code(&self) -> Option<u8> {
        self.base_mod_code
    }

    pub fn canonical_base(&self) -> CanonicalBase {
        self.canonical_base
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
                if reverse_strand {
                    cb.complement()
                } else {
                    cb
                }
            };

            if c != canonical_base {
                return Err(BaseModsError::ModifierMismatch(
                    canonical_base,
                    if reverse_strand { '-' } else { '+' },
                    b as char
                ));
            }
        }
        Ok(Self {
            canonical_base,
            reverse_strand,
            base_mod_code,
            chebi_code,
        })
    }

    pub fn reverse_strand(&self) -> bool {
        self.reverse_strand
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
