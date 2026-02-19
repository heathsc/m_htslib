use std::{ffi::CStr, mem::ManuallyDrop, ops::Range};

use smallvec::SmallVec;

use crate::{
    BaseModsError,
    sam::{BamAuxTagType, BamAuxVal, BamRec},
};

use super::{ModIter, ModUnit, ModUnitIterData, Modification, delta::DeltaItr};

const N_MODS: usize = 4;

/// This is an ugly hack - I agree...
///
/// We want to have a persistent storage for a Vec<ModUnitIterData<'a, 'b>> so
/// that we can use it for subsequent calls to generate an iterator, avoiding the
/// need to allocate a new Vec each time. THe problem is that if we include it in
/// MMParse struct itself, 'a refers to the lifetime of the MMParse struct itself,
/// and 'b refers to the BamRec we are working on. This makes it almost impossible to
/// use, but this is not necessary because we only use the storage while the generated
/// iterator is in scope. For the next parse, the vector will be reset to empty, but
/// the compiler does not know that so we end up with the lifetimes of MMParse, the
/// BamRec and the iterator all linked together.
///
/// To avoid this, we take advantage of the fact that we can create a Vec<ModUnitIterData>
/// with a given capacity without specifiying the lifetimes (as this does not change its size).
/// We can then take the allocated pointer and capacity from the Vec and store them for later
/// use in MMParseWork. Note we use ManuallyDrop so that the pinter is not deallocated when
/// the originally vector goes out of scope.
///
/// At the start of each [MMParse::mk_pos_iter] call, we can then construct a new empty
/// Vec using the ptr and capacity stored in the [MMParseWork] struct. When the Vec has
/// been filled, we again use ManuallyDrop to prevent dealloaction of the memory, and
/// update the [MMParseWork] struct in case of any changes.
///
/// Extreme care must be taken in using this struct, which is why it is private to this
/// module and should not be made public!
struct MMParseWork {
    ptr: *mut u8,
    cap: usize,
}

impl Default for MMParseWork {
    fn default() -> Self {
        let v: Vec<ModUnitIterData<'_, '_>> = Vec::with_capacity(N_MODS);
        let mut me = ManuallyDrop::new(v);
        let (ptr, cap) = (me.as_mut_ptr() as *mut u8, me.capacity());
        Self { ptr, cap }
    }
}

impl Drop for MMParseWork {
    fn drop(&mut self) {
        let v =
            unsafe { Vec::from_raw_parts(self.ptr as *mut ModUnitIterData<'_, '_>, 0, self.cap) };
        drop(v);
    }
}

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

    data_vec: Vec<Modification>,

    ml_data: Vec<u8>,

    m_data: MMParseWork,
}

impl MMParse {
    pub fn new() -> Self {
        Self::default()
    }

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
    where
        'a: 'b,
    {
        self.parse_tags(rec, mm)?;

        // Make vec of modifications (indexed by mod_unit, mod within unit) that we will be looking
        // at from this record.   For each selected mod_unit we generate an iterator of delta values
        // and probabilities.

        self.current_select.clear();
        let s = self.selection.as_deref();

        let mut mdata = unsafe {
            Vec::from_raw_parts(
                self.m_data.ptr as *mut ModUnitIterData<'_, '_>,
                0,
                self.m_data.cap,
            )
        };

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
                let ml_iter = MlIter::make(ml_values, rec.is_reversed(), unit.n_mods());
                let mut mod_iter = delta_iter.zip(ml_iter);
                let data = mod_iter.next();
                mdata.push(ModUnitIterData::make(mod_iter, unit, data))
            }
        }
        let seq_iter = rec.seq();
        let mut mdata = ManuallyDrop::new(mdata);
        self.m_data.ptr = mdata.as_mut_ptr() as *mut u8;
        self.m_data.cap = mdata.capacity();

        Ok(ModIter::make(
            &mut self.data_vec,
            &self.current_select,
            mdata,
            seq_iter,
            rec.is_reversed(),
        ))
    }

    /// Use the standard MM and ML tags
    pub fn mod_iter<'a, 'b>(
        &'a mut self,
        rec: &'b BamRec,
    ) -> Result<Option<ModIter<'a, 'b>>, BaseModsError>
    where
        'a: 'b,
    {
        self.mod_iter_internal(rec, false)
    }
    
    /// Use the alternate mm and ml tags
    pub fn mod_iter_alt<'a, 'b>(
        &'a mut self,
        rec: &'b BamRec,
    ) -> Result<Option<ModIter<'a, 'b>>, BaseModsError>
    where
        'a: 'b,
    {
        self.mod_iter_internal(rec, true)
    }
    
    fn mod_iter_internal<'a, 'b>(
        &'a mut self,
        rec: &'b BamRec,
        alt_tags: bool,
    ) -> Result<Option<ModIter<'a, 'b>>, BaseModsError>
    where
        'a: 'b,
    {
        self.clear();

        // Look for MM, ML and MN tags in rec
        if let Some((mm, mn)) = self.find_mod_tags(rec, alt_tags).map_err(|e| {
            BaseModsError::General(format!(
                "Error processing MM/ML/MN tags for read {:?}: {e}",
                rec.qname()
            ))
        })? {
            // Check MN tag (if it exists) reports the same as the sequence length, otherwise we cannot trust the MM tags.
            if let Some(n) = mn
                && n as usize != rec.seq_len()
            {
                return Err(BaseModsError::MMSeqMismatch);
            }
            self.mk_pos_iter(rec, mm.to_bytes()).map(Some)
        } else {
            Ok(None)
        }
    }

    fn find_mod_tags<'a>(
        &mut self,
        rec: &'a BamRec,
        alt_tags: bool,
    ) -> Result<Option<(&'a CStr, Option<i64>)>, BaseModsError> {
        let mut ml_len = None;
        let mut mm_val = None;
        let mut mn_len = None;

        let (mm_tag_pattern, ml_tag_pattern) = if alt_tags {
            ("mm", "ml")
        } else {
            ("MM", "ML")
        };
        for item in rec.aux_tags() {
            let item = item?;
            let tag = item.id()?;

            match tag {
                s if s == mm_tag_pattern => {
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
                s if s == ml_tag_pattern => {
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

pub(super) struct MlIter<'a, T: 'a> {
    v: &'a [T],
    sz: usize,
    ix: usize,
    decr: usize,
    i: usize,
}

impl<'a, T> MlIter<'a, T> {
    fn make(v: &'a [T], reverse: bool, sz: usize) -> Self {
        let (ix, decr, i) = if reverse && v.len() >= sz {
            (v.len(), sz, 1)
        } else {
            (sz, 0, 0)
        };
        Self { v, sz, ix, decr, i }
    }
}

impl<'a, T> Iterator for MlIter<'a, T> {
    type Item = &'a [T];

    fn next(&mut self) -> Option<Self::Item> {
        if self.v.len() < self.sz {
            None
        } else {
            self.ix -= self.decr;
            let (a, b) = self.v.split_at(self.ix);
            let f = [a, b];
            self.v = f[1 - self.i];
            Some(f[self.i])
        }
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
