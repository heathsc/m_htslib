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
    use crate::{hts::HtsFile, kstring::KString, sam::CigarBuf, sam::SamHdr};
    #[allow(unused)]
    use std::io::Write;

    #[test]
    fn test_parse() {
        let mut h =
            HtsFile::open(c"test/realn01.sam", c"r").expect("Failed to read test/realn01.sam");
        let mut hdr = SamHdr::read(&mut h).expect("Failed to read header");

        let mut ks = KString::new();
        let mut b = BamRec::new();
        let mut cb = CigarBuf::new();

        b.parse(b"read_id1\t147\t0000000F\t412\t49\t11M\t=\t193\t-380\tCTGCAATACGC\tAAFJFFBCAFF\tNM:i:0\tRG:Z:rg\txs:B:s,-32,400,21", &mut hdr, &mut ks, &mut cb).expect("Error parsing SAM record");

        let mut itr = b.seq();
        let mut v: Vec<u8> = Vec::with_capacity(5);

        write!(v, "{}", itr.next().unwrap()).unwrap();
        write!(v, "{}", itr.next().unwrap()).unwrap();
        write!(v, "{}", itr.next().unwrap()).unwrap();
        write!(v, "{}", itr.nth(3).unwrap()).unwrap();
        write!(v, "{}", itr.last().unwrap()).unwrap();

        assert_eq!(&v, b"CTGTC");

        let mut itr = b.seq().rev();
        v.clear();
        write!(v, "{}", itr.next().unwrap()).unwrap();
        write!(v, "{}", itr.next().unwrap()).unwrap();
        write!(v, "{}", itr.next().unwrap()).unwrap();
        write!(v, "{}", itr.nth(3).unwrap()).unwrap();
        write!(v, "{}", itr.nth(2).unwrap()).unwrap();
        write!(v, "{}", itr.next().unwrap()).unwrap();

        assert_eq!(std::str::from_utf8(&v).unwrap(), "CGCATC");
        assert_eq!(itr.next(), None)
    }
}
