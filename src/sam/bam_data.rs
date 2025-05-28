mod bam_data_impl;
pub mod bd_state_impl;
pub mod sections;
mod validate;
pub mod writer;

pub use sections::*;
pub use writer::*;

use std::collections::HashSet;

use crate::{SamError, kstring::KString};

#[derive(Debug)]
pub struct BamData {
    state: BDState,
    data: KString,
    tmp_data: KString,
    mask: BDMask,
    section: Option<BDSection>,
    last_error: Option<SamError>,
    hash: HashSet<[u8; 2]>,
}

#[derive(Debug, Default, Copy, Clone)]
struct BDState {
    n_cigar_elem: u32,
    qname_len: u16,
    extra_nul: u8,
    seq_len: i32, // Length in bases (= length of quality data record)
}

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

        eprintln!("Getting writer for Cigar");
        {
            let mut w = bd.writer(BDSection::Cigar).cigar_writer().unwrap();
            let mut cb = CigarBuf::new();
            cb.parse("1S14M").unwrap();
            w.write_elems(cb.as_elems()).unwrap();
        }
        
        eprintln!("Getting writer for Seq");
        {
            let mut w = bd.writer(BDSection::Seq).seq_writer().unwrap();
            w.write_seq(b"ACCGTTCTTGAACAA").unwrap();
        }
        
        eprintln!("Getting writer for Qual");
        {
            let mut w = bd.writer(BDSection::Qual);
            w.write_all(&[32,40,31,17,6,33,31,29,30,28,30,31,32,33,34]).unwrap();
        }
        
        bd.validate().expect("Validation error");
        panic!("OOOOK!");
    }
}
