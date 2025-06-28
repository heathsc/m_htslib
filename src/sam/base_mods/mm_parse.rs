use std::{ffi::CStr, ops::Range};

use smallvec::SmallVec;

use crate::{
    BaseModsError,
    sam::{BamAuxTagType, BamAuxVal, BamRec},
};

use super::{
    MlIter, ModIter, ModUnit, ModUnitIterData, Modification, delta::DeltaItr,
};

const N_MODS: usize = 4;
// Size of backing store for ModUnits in a MM tag (N_MODS2).  Note that we are using SmallVec so we
// can have more mods than this, but if the number of greater it will be allocated and stored on the heap.
#[derive(Default)]
pub struct MMParse {
    // Information on modifications parsed from BAM record
    mod_units: SmallVec<[ModUnit; N_MODS]>,
    // To avoid unnecessary allocations we never reduce the size of the mod_units vector (as this
    // would drop the ModUnit elements which would deallocate their storage).  Instead we use n_units
    // to track the number of ModUnits from the current record
    n_units: usize,

    // Optional vector of modifications for selection
    selection: Option<Vec<Modification>>,

    // (i, j) where the selected mod is self.mod_units[i].mods[j]
    current_select: Vec<(usize, usize)>,

    data_vec: Vec<(u8, Modification)>,

    ml_data: Vec<u8>,
}

impl MMParse {
    fn new_unit(&mut self) -> &mut ModUnit {
        if self.n_units == self.mod_units.len() {
            self.mod_units.push(Default::default())
        }
        let m = &mut self.mod_units[self.n_units];
        m.clear();
        self.n_units += 1;
        m
    }

    pub fn clear(&mut self) {
        self.n_units = 0;
        self.current_select.clear();
        self.ml_data.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.n_units == 0
    }

    pub fn n_units(&self) -> usize {
        self.n_units
    }

    pub fn n_mods(&self) -> usize {
        self.mod_units[..self.n_units]
            .iter()
            .fold(0, |s, m| s + m.n_mods())
    }

    pub fn units(&self) -> &[ModUnit] {
        &self.mod_units[..self.n_units]
    }

    pub fn set_selection(&mut self, mstr: &[&str]) -> Result<(), BaseModsError> {
        let mut v = self.selection.take().unwrap_or_default();
        for s in mstr {
            let m = s.as_bytes();
            let ix = Modification::from_u8_slice(m, &mut v)?;
            if ix != m.len() {
                return Err(BaseModsError::TrailingGarbageModDesc);
            }
        }
        self.selection = Some(v);
        Ok(())
    }

    pub fn clear_selection(&mut self) {
        self.selection = None
    }

    fn mk_pos_iter<'a, 'b>(
        &'a mut self,
        rec: &'b BamRec,
        mm: &'b [u8],
    ) -> Result<ModIter<'a, 'b>, BaseModsError>
   where 'a : 'b
    {
        self.parse_tags(rec, mm)?;

        // Make vec of modifications (indexed by mod_unit, mod within unit) that we will be looking
        // at from this record.   For each selected mod_unit we generate an iterator of delta values
        // and probabilities.

        self.current_select.clear();
        let s = self.selection.as_deref();
        let mut mdata = Vec::with_capacity(self.n_units);
        for unit in &self.mod_units[0..self.n_units] {
            if unit.data().is_none() {
                continue;
            }
            let mut used = false;
            let i = mdata.len();
            for (j, md) in unit.mods().iter().enumerate() {
                if s.map(|v| v.contains(md)).unwrap_or(true) {
                    self.current_select.push((i, j));
                    used = true
                }
            }
            // If this ModUnit is required, generate iterators
            if used {
                let data = unit.data().unwrap();
                let delta_values = &mm[data.mm_data_range().clone()];
                let delta_iter = DeltaItr::new(
                    delta_values,
                    data.n_delta(),
                    data.first_delta(),
                    rec.is_reversed(),
                );
                let ml_values = &self.ml_data[data.ml_data_range().clone()];
                let ml_iter = mk_ml_iter(ml_values, rec.is_reversed(), unit.n_mods());
                let mut mod_iter = delta_iter.zip(ml_iter);
                let data = mod_iter.next();
                mdata.push(ModUnitIterData::make(mod_iter, unit, data))
            }
        }
        let seq_iter = rec.seq();
        Ok(ModIter::make(
            &mut self.data_vec,
            &self.current_select,
            mdata,
            seq_iter,
            rec.is_reversed(),
        ))
    }

    pub fn mod_iter<'a, 'b>(
        &'a mut self,
        rec: &'b BamRec,
    ) -> Result<Option<ModIter<'a, 'b>>, BaseModsError> 
    where 'a : 'b
    {
        self.clear();

        // Look for MM, ML and MN tags in rec
        if let Some((mm, mn)) = self.find_mod_tags(rec).map_err(|e| {
            BaseModsError::General(format!(
                "Error processing MM/ML/MN tags for read {:?}: {e}",
                rec.qname()
            ))
        })? {
            // Check MN tag (if it exists) reports the same as the sequence length, otherwise we cannot trust the MM tags.
            if let Some(n) = mn {
                if n as usize != rec.seq_len() {
                    return Err(BaseModsError::MMSeqMismatch);
                }
            }
            self.mk_pos_iter(rec, mm.to_bytes()).map(Some)
        } else {
            Ok(None)
        }
    }

    fn find_mod_tags<'a>(
        &mut self,
        rec: &'a BamRec,
    ) -> Result<Option<(&'a CStr, Option<i64>)>, BaseModsError> {
        let mut ml_len = None;
        let mut mm_val = None;
        let mut mn_len = None;

        for item in rec.aux_tags() {
            let item = item?;
            let tag = item.id()?;

            match tag {
                "MM" => {
                    let mm = if let BamAuxVal::String(v) = item.get_val()? {
                        v
                    } else {
                        return Err(BaseModsError::MMTagNotString); // This should have been caught before
                    };

                    if mm_val.is_some() {
                        return Err(BaseModsError::MultipleMMTags);
                    }
                    mm_val = Some(mm);
                }
                "ML" => {
                    if !matches!(
                        item.get_type()?,
                        (BamAuxTagType::Array, Some(BamAuxTagType::UInt8))
                    ) {
                        return Err(BaseModsError::MLTagNotUInt8Array);
                    }
                    let len = if let BamAuxVal::IntArray(v) = item.get_val()? {
                        for i in v {
                            self.ml_data.push(i as u8)
                        }
                        self.ml_data.len()
                    } else {
                        panic!("Internal error");
                    };
                    if ml_len.is_some() {
                        return Err(BaseModsError::MultipleMLTags);
                    }
                    ml_len = Some(len);
                }
                "MN" => {
                    if let BamAuxVal::Int(x) = item.get_val()? {
                        if mn_len.is_some() {
                            return Err(BaseModsError::MultipleMNTags);
                        }
                        mn_len = Some(x)
                    } else {
                        return Err(BaseModsError::MNTagNotInteger);
                    }
                }
                _ => (),
            }
        }
        let x = if let Some(mm) = mm_val
            && ml_len.is_some()
        {
            Some((mm, mn_len))
        } else {
            None
        };

        Ok(x)
    }

    /// Parse MM/ML tags so that later we can iterate through modified positions
    fn parse_tags<'a>(&mut self, rec: &'a BamRec, mm: &'a [u8]) -> Result<(), BaseModsError> {
        let mm = check_mm_termination(mm)?;

        // Get base counts from sequence
        let base_counts = count_seq_bases(rec);

        let mut total_deltas = 0;

        for m in mm.split(|c| *c == b';') {
            let unit = self.new_unit();

            unit.parse_modifications_from_u8_slice(m, &base_counts, rec.is_reversed())?;

            // Check if any data are available from this MM tag unit
            if unit.n_mods() == 0 || unit.data().is_none() {
                continue;
            }

            let n_mods = unit.n_mods();
            let mdata = unit.data_mut().unwrap();
            // Adjust data range by starting point
            let offset = unsafe { m.as_ptr().offset_from(mm.as_ptr()) };
            assert!(offset >= 0);
            let offset = offset as usize;

            let Range { start, end } = mdata.mm_data_range();
            mdata.set_mm_data_range(start + offset..end + offset);

            // Find data range for ML tag probabilities
            let n_prob = mdata.n_delta() * n_mods;
            mdata.set_ml_data_range(total_deltas..total_deltas + n_prob);
            total_deltas += n_prob;
        }

        if total_deltas != self.ml_data.len() {
            Err(BaseModsError::MMandMLLenMismatch)
        } else {
            Ok(())
        }
    }
}

fn mk_ml_iter(ml: &[u8], reverse: bool, n: usize) -> MlIter {
    if reverse {
        Box::new(ml.rchunks(n))
    } else {
        Box::new(ml.chunks(n))
    }
}

/// Check MM tag is terminated with a ';' and, if so, remove final ';'
fn check_mm_termination(v: &[u8]) -> Result<&[u8], BaseModsError> {
    if let Some((lst, rem)) = v.split_last() {
        if *lst != b';' {
            return Err(BaseModsError::MMTagMissingTerminator);
        }
        Ok(rem)
    } else {
        Err(BaseModsError::EmptyMMTag)
    }
}

/// Calculate base counts from Bam record sequence data
fn count_seq_bases(rec: &BamRec) -> [u32; 16] {
    let seq_it = rec.seq();

    let mut ct = [0; 16];
    let mut n = 0;
    for b in seq_it {
        ct[b.as_u8() as usize] += 1;
        n += 1;
    }

    // All bases count as N for base mods
    ct[15] = n;

    ct
}
