mod bam_data_impl;
pub mod bd_state_impl;
pub mod sections;
mod validate;
pub mod writer;

pub use sections::*;
pub use writer::*;

use std::collections::HashSet;

use crate::{SamError, kstring::MString};

#[derive(Debug)]
pub struct BamData {
    state: BDState,
    data: MString,
    tmp_data: MString,
    mask: BDMask,
    section: Option<BDSection>,
    last_error: Option<SamError>,
    hash: Option<HashSet<[u8; 2]>>,
}

#[derive(Debug, Default, Copy, Clone)]
pub(super) struct BDState {
    n_cigar_elem: u32,
    qname_len: u16,
    extra_nul: u8,
    seq_len: i32, // Length in bases (= length of quality data record)
}

/* 

/// Actions for ['BamRec'] ['swap_data']
/// 
/// This swaps the bam data segment from a BamRec with the data from a BamData. The action
/// parameters describes what is done with the data segment recovered by the BamRec
///
/// [`swap_data`]: BamRec::swap_data
#[derive(Debug, Copy, Clone)]
pub(super) enum BDAction {
    Ignore,                    // The BamRec is cleared (empty record)
    Replace(BDState, BDMask),  // The metadata for the data segment is copied from the BamData
    Merge(BDState, BDMask),    // The BamData and BamRec data segments are merged. Individual segments replaced except the Aux segment which is merged.
}

*/

#[cfg(test)]
mod tests {
    #![allow(unused)]

    use std::io::Write;
    
    use crate::sam::CigarBuf;
    
    use super::*;

    #[test]
    fn bam_data() {
        let mut bd = BamData::default();

        eprintln!("Getting writer for QName");
        {
            let mut w = bd.writer(BDSection::QName);
            w.write_all(b"Test").unwrap();
        }
        assert!(bd.last_error().is_none());
        
        eprintln!("Getting writer for Cigar");
        {
            let mut w = bd.writer(BDSection::Cigar).cigar_writer().unwrap();
            let mut cb = CigarBuf::new();
            w.write_cigar(b"1S14M").unwrap();
        }
        assert!(bd.last_error().is_none());
        
        eprintln!("Getting writer for Seq");
        {
            let mut w = bd.writer(BDSection::Seq).seq_writer().unwrap();
            w.write_seq(b"ACCGTTCTTGAACAA").unwrap();
        }
        assert!(bd.last_error().is_none());
        
        eprintln!("Getting writer for Qual");
        {
            let mut w = bd.writer(BDSection::Qual);
            w.write_all(&[32,40,31,17,6,33,31,29,30,28,30,31,32,33,34]).unwrap();
        }
        assert!(bd.last_error().is_none());
        
        eprintln!("Getting writer for Aux");
        {
            let mut w = bd.writer(BDSection::Aux).aux_writer().unwrap();
            w.write_aux(b"xa:i:4\txb:Z:Hi\txc:A:v\txd:B:c,41,8").unwrap();
        }
        
        bd.validate().expect("Validation error");
    }
    
    #[test]
    fn bam_data_rev() {
        let mut bd = BamData::default();

        eprintln!("Getting writer for Aux");
        {
            let mut w = bd.writer(BDSection::Aux).aux_writer().unwrap();
            w.write_aux(b"xa:i:4\txb:Z:Hi\txc:A:v\txd:B:c,41,8").unwrap();
        }
        assert!(bd.last_error().is_none());
        
        eprintln!("Getting writer for Qual");
        {
            let mut w = bd.writer(BDSection::Qual);
            w.write_all(&[32,40,31,17,6,33,31,29,30,28,30,31,32,33,34]).unwrap();
        }
        assert!(bd.last_error().is_none());
        
        eprintln!("Getting writer for Seq");
        {
            let mut w = bd.writer(BDSection::Seq).seq_writer().unwrap();
            w.write_seq(b"ACCGTTCTTGAACAA").unwrap();
        }
        assert!(bd.last_error().is_none());
        
        eprintln!("Getting writer for Cigar");
        {
            let mut w = bd.writer(BDSection::Cigar).cigar_writer().unwrap();
            let mut cb = CigarBuf::new();
            w.write_cigar(b"1S14M").unwrap();
        }
        assert!(bd.last_error().is_none());

        eprintln!("Getting writer for QName");
        {
            let mut w = bd.writer(BDSection::QName);
            w.write_all(b"Test").unwrap();
        }     
        
        bd.validate().expect("Validation error");
    }
}
