pub mod base_mods_error;
pub mod bases;
mod delta;
pub mod mm_parse;
pub mod mod_iter;
pub mod mod_unit;
pub mod modification;

pub use bases::*;
pub use mm_parse::*;
pub use mod_iter::*;
pub use mod_unit::*;
pub use modification::*;

#[cfg(test)]
mod tests {
    #![allow(unused)]

    use super::*;

    use crate::{
        hts::{HtsFile, ReadRec},
        sam::{BamRec, SamHdr, SamReader, base_mods::MMParse},
    };

    #[test]
    fn test_parse_meth() {
        let mut h = HtsFile::open(c"test/long_read_meth.bam", c"r")
            .expect("Failed to read test/long_read_meth.bam");
        let hdr = SamHdr::read(&mut h).unwrap();

        let mut rdr = SamReader::new(&mut h, &hdr);
        let mut rec = BamRec::new();
        rdr.read_rec(&mut rec).unwrap().unwrap();

        let mut mm = MMParse::default();
        let mut it = mm.mod_iter(&rec).unwrap().unwrap();
        
        let mut n = 0;
        while let Some(x) = it.next_pos() {
            if !x.data().is_empty() {
                eprintln!("{x:?}");
                n += 1;
            }
        }
        assert_eq!(n, 1432);
    }
}
