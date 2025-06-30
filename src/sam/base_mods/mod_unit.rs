use std::ops::Range;

use crate::BaseModsError;

use super::{Modification, delta::parse_mm_count_fwd};

/// A ModUnit corresponds to one element in a MM tag, which might contain information on multiple
/// modifications.  For example, the tag 'C+m,5,12,0;C+h,5,12,0' has two ModUnits, one for a
/// 5mC mod and the other for a 5hmC mod.  However, the tag 'C+mh,5,12,0' has a single ModUnit
/// which contains information on 5mC and 5hmC mods.  Note the two examples given above are
/// functionally equivalent, but are handled differently internally.
#[derive(Default)]
pub struct ModUnit {
    mods: Vec<Modification>,
    data: Option<ModUnitData>,
}

impl ModUnit {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.mods.clear();
        self.data = None;
    }

    pub fn data(&self) -> Option<&ModUnitData> {
        self.data.as_ref()
    }
    
    pub fn data_mut(&mut self) -> Option<&mut ModUnitData> {
        self.data.as_mut()
    }

    pub fn mods(&self) -> &[Modification] {
        &self.mods
    }

    pub fn n_mods(&self) -> usize {
        self.mods.len()
    }

    pub(super) fn parse_modifications_from_u8_slice(
        &mut self,
        v: &[u8],
        base_counts: &[u32],
        read_reversed: bool,
    ) -> Result<(), BaseModsError> {
        self.mods.clear();
        let mut ix = Modification::from_u8_slice(v, &mut self.mods)?;
        assert!(!self.mods.is_empty());
        let canonical_base = self.mods[0].canonical_base();

        let (implicit, ix1) = check_implicit(&v[ix..])?;
        ix += ix1;

        if ix == v.len() {
            // No data on this modification in this read
            return Ok(());
        }

        // Count number of delta counts (essentially we count the commas) and check general validity
        let (n_delta, total_seq, first_delta, ix1) = count_delta_entries(&v[ix..])?;

        // Check the bases in the MM tag are within the sequence data
        let base_count = base_counts[if read_reversed {
            canonical_base.complement() as usize
        } else {
            canonical_base as usize
        }];
        
        if base_count < total_seq {
            return Err(BaseModsError::MMSeqMismatch);
        }

        let mm_data_range = Range {
            start: ix + ix1,
            end: v.len(),
        };
        let first_delta = if read_reversed {
            base_count - total_seq
        } else {
            first_delta
        };
        self.data = Some(ModUnitData {
            mm_data_range,
            ml_data_range: 0..0,
            n_delta,
            first_delta,
            implicit,
        });
        Ok(())
    }
}

/// Data associated with a [ModUnit]
///
/// mm_data_range and ml_data_range are the indexes of a subslice of the [u8] with the data
/// for the MM and ML tags within the BAM record we are working on.
///
/// n_delta has the number of delta entries from the MM tag and first_delta is (as you would expect)
/// the first delta entry.
///
/// The implicit flag is read from the MM tag and is true if the implicit indicator is '.' or
/// missing, and false if the indicator is '?'.
#[derive(Default)]
pub struct ModUnitData {
    mm_data_range: Range<usize>,
    ml_data_range: Range<usize>,
    n_delta: usize,
    first_delta: u32,
    implicit: bool,
}

impl ModUnitData {
    pub fn new() -> Self {
        Self::default()
    }

    pub(super) fn implicit(&self) -> bool {
        self.implicit
    }

    pub(super) fn mm_data_range(&self) -> &Range<usize> {
        &self.mm_data_range
    }

    pub(super) fn n_delta(&self) -> usize {
        self.n_delta
    }
    
    pub(super) fn first_delta(&self) -> u32 {
        self.first_delta
    }

    pub(super) fn set_mm_data_range(&mut self, r: Range<usize>) {
        self.mm_data_range = r
    }
    
    pub(super) fn set_ml_data_range(&mut self, r: Range<usize>) {
        self.ml_data_range = r
    }

    pub(super) fn ml_data_range(&self) -> &Range<usize> {
        &self.ml_data_range
    }
}

fn check_implicit(v: &[u8]) -> Result<(bool, usize), BaseModsError> {
    if v.is_empty() {
        // None of these mods exist for this read
        Ok((true, 0))
    } else {
        match v.split_at(1) {
            (&[b'.'], _) => Ok((true, 1)),
            (&[b'?'], _) => Ok((false, 1)),
            (&[b','], _) => Ok((true, 0)),
            (c, _) if c[0].is_ascii_alphanumeric() => {
                Err(BaseModsError::BadImplicitMMCode2(c[0] as char))
            }
            _ => Err(BaseModsError::BadImplicitMMCode),
        }
    }
}

/// Verify MM deltas and count number of deltas + total number of canonical bases accounts for.
/// Will panic if v is empty.  Returns a tuple with the number of deltas, the base count, the
/// first delta and the index to the *second* delta value (including leading comma) if it exists.
fn count_delta_entries(v: &[u8]) -> Result<(usize, u32, u32, usize), BaseModsError> {
    let (first_delta, a) = parse_mm_count_fwd(v)?;

    let mut ix = a[0];
    let mut n_delta = 1;
    let mut total_seq: u32 = first_delta + 1;
    let ret = ix;
    while ix < v.len() {
        let (delta, ix1) = parse_mm_count_fwd(&v[ix..])?;
        n_delta += 1;
        total_seq = total_seq
            .checked_add(delta + 1)
            .ok_or(BaseModsError::BaseCountOverflow)?;
        ix += ix1[0];
    }

    Ok((n_delta, total_seq, first_delta, ret))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_delta_entries() -> Result<(), BaseModsError> {
        assert_eq!(
            count_delta_entries(",32,5,19,2,213".as_bytes())?,
            (5, 276, 32, 3)
        );
        Ok(())
    }

    #[test]
    fn test_count_delta_entries_2() -> Result<(), BaseModsError> {
        assert_eq!(count_delta_entries(",32".as_bytes())?, (1, 33, 32, 3));
        Ok(())
    }
    
    #[test]
    fn test_parse_modifications() -> Result<(), BaseModsError> {
        let mut mod_unit1 = ModUnit::new();
        let base_counts = [0, 0, 180, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        mod_unit1.parse_modifications_from_u8_slice(
            "C+mh?,4,10,154".as_bytes(),
            &base_counts,
            false,
        )?;
        assert_eq!(mod_unit1.mods().len(), 2);
        let mut mod_unit2 = ModUnit::new();
        mod_unit2.parse_modifications_from_u8_slice(
            "C+76792?,2,2,9".as_bytes(),
            &base_counts,
            false,
        )?;
        assert_eq!(mod_unit2.mods().len(), 1);
        assert_eq!(mod_unit1.mods()[1], mod_unit2.mods()[0]);
        let s = format!("{}", mod_unit1.mods()[1]);
        assert_eq!(s.as_str(), "C+h");
        let s = format!("{:#}", mod_unit1.mods()[1]);
        assert_eq!(s.as_str(), "5hmC+");
        let mdata = mod_unit1.data().unwrap();
        assert_eq!(mdata.n_delta(), 3);
        assert!(!mdata.implicit());
        assert_eq!(mdata.mm_data_range(), &(7..14));
        Ok(())
    }
}