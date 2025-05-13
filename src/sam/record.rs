pub mod bam1;

use bam1::*;

#[derive(Clone, Default, Debug)]
pub struct BamRec {
    inner: bam1_t,
}

mod tests {
    #[allow(unused)]
    use super::*;
    #[allow(unused)]
    use crate::{sam::SamHdr, hts::HtsFile, kstring::KString, sam::CigarBuf};
    
    #[test]
    fn test_parse() {
    
        let mut h = HtsFile::open(c"test/realn01.sam", c"r").expect("Failed to read test/realn01.sam");
        let mut hdr = SamHdr::read(&mut h).expect("Failed to read header");
        
        let mut ks = KString::new();
        let mut b = BamRec::new();
        let mut cb = CigarBuf::new();
        
        b.parse(b"read_id1\t147\t0000000F\t412\t49\t8M\t=\t193\t-380\tCAGCAAGC\tAAFJFFBC\tNM:i:0\tRG:Z:rg", &mut hdr, &mut ks, &mut cb).expect("Error parsing SAM record");
        
    }
}