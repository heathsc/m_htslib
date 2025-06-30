use crate::{base::Base, sam::SeqIter};

use super::{ModUnit, Modification, delta::DeltaItr};

pub(super) type MlIter<'a> = Box<dyn Iterator<Item = &'a [u8]> + 'a>;
pub(super) type MdIter<'a> = std::iter::Zip<DeltaItr<'a>, MlIter<'a>>;

#[derive(Copy, Clone, Debug)]
pub struct ModIterItem<'a> {
    seq_base: Base,
    data: &'a Vec<Modification>,
}

impl<'a> ModIterItem<'a> {
    pub fn seq_base(&self) -> Base {
        self.seq_base
    }

    pub fn data(&self) -> &[Modification] {
        self.data
    }
}

pub enum ModUnitIterValue<'a> {
    Explicit(&'a [u8]),
    Implicit,
    Missing,
}

// 'a from MMParse, 'b from BamRec
pub struct ModUnitIterData<'a, 'b> {
    mod_iter: MdIter<'b>,
    mod_unit: &'a ModUnit,
    data: Option<(u32, &'b [u8])>,
    current_value: ModUnitIterValue<'b>,
    done: bool,
}

impl<'a, 'b> ModUnitIterData<'a, 'b> {
    pub(super) fn make(
        mod_iter: MdIter<'b>,
        mod_unit: &'a ModUnit,
        data: Option<(u32, &'b [u8])>,
    ) -> Self {
        Self {
            mod_iter,
            mod_unit,
            data,
            current_value: ModUnitIterValue::Missing,
            done: false,
        }
    }
}

/// Struct for iterating over a combination of the bases on the read and any modified bases as
/// read in from MM/ML tags in the BAM record.
///
/// Note that 'a from MMParse, 'b from BamRec as data_vec and select are references to data held
/// in an MMParse struct while seq_iter is an iterator over the bases in the read.
pub struct ModIter<'a, 'b> {
    data_vec: &'a mut Vec<Modification>,
    select: &'a [(usize, usize)],
    unit_iters: Vec<ModUnitIterData<'a, 'b>>,
    seq_iter: SeqIter<'b>,
    finished: bool,
    reversed: bool,
}

impl<'a, 'b> ModIter<'a, 'b> {
    pub(super) fn make(
        data_vec: &'a mut Vec<Modification>,
        select: &'a [(usize, usize)],
        unit_iters: Vec<ModUnitIterData<'a, 'b>>,
        seq_iter: SeqIter<'b>,
        reversed: bool,
    ) -> Self {
        let finished = unit_iters.is_empty();
        Self {
            data_vec,
            select,
            unit_iters,
            seq_iter,
            finished,
            reversed,
        }
    }

    pub fn next_pos(&mut self) -> Option<ModIterItem> {
        if self.finished {
            None
        } else if let Some(sbase) =
            // Get next sequence base from read (reverse complement if necessary)
            self.seq_iter.next()
        {
            // This is where we store any matching mods
            self.data_vec.clear();

            // For each mod unit, clear flags to indicate that we haven't yet processed it
            // for this site
            for unit in self.unit_iters.iter_mut() {
                unit.done = false;
                unit.current_value = ModUnitIterValue::Missing;
            }

            let sbase1 = if self.reversed {
                sbase.complement()
            } else {
                sbase
            };
            // Go through selected modifications to see if they match
            for (i, j) in self.select {
                let unit = &mut self.unit_iters[*i];
                // If we've not processed this unit so far, check if we have modification(s) at this
                // sequence base
                if !unit.done {
                    // Only look at mods where the sequenced base matches the canonical base
                    if (sbase1.as_u8() & unit.mod_unit.mods()[*j].canonical_base().as_u8()) != 0 {
                        // Check if we need to skip this base
                        if let Some((delta, probs)) = unit.data.take() {
                            if delta == 0 {
                                // We are at the modified site.  Take the probability value and
                                // get the next one (if present)
                                unit.current_value = ModUnitIterValue::Explicit(probs);
                                unit.data = unit.mod_iter.next().or(Some((u32::MAX, &[])))
                            } else {
                                unit.data =
                                    Some((if probs.is_empty() { delta } else { delta - 1 }, probs));

                                if unit.mod_unit.data().unwrap().implicit() {
                                    unit.current_value = ModUnitIterValue::Implicit
                                }
                            }
                        }
                    }
                    unit.done = true;
                }
                // unit.current_value will be Some(p) where p is a vector of probabilities for
                // mods at this site.
                match unit.current_value {
                    ModUnitIterValue::Explicit(p) => {
                        let mut m = unit.mod_unit.mods()[*j];
                        m.set_ml_value(p[*j]);
                        self.data_vec.push(m)
                    }
                    ModUnitIterValue::Implicit => {
                        let mut m = unit.mod_unit.mods()[*j];
                        m.set_implicit_ml_value();
                        self.data_vec.push(m)
                    }
                    ModUnitIterValue::Missing => {}
                }
            }

            Some(ModIterItem {
                seq_base: sbase,
                data: self.data_vec,
            })
        } else {
            self.finished = true;
            None
        }
    }
}
