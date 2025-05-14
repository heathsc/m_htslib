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
    use crate::{hts::HtsFile, kstring::KString, sam::{CigarBuf, SamHdr, SequenceIter}};
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
        assert_eq!(itr.next(), None);

        let mut itr = b.seq();
        let mut v: Vec<u8> = Vec::with_capacity(5);

        write!(v, "{}", itr.next().unwrap()).unwrap();
        write!(v, "{}", itr.next().unwrap()).unwrap();
        write!(v, "{}", itr.next().unwrap()).unwrap();
        write!(v, "{}", itr.next_back().unwrap()).unwrap();
        write!(v, "{}", itr.next_back().unwrap()).unwrap();
        write!(v, "{}", itr.next().unwrap()).unwrap();
        
        assert_eq!(std::str::from_utf8(&v).unwrap(), "CTGCGC");
        
        let s: String = b.seq().map(|c| c.as_char()).collect();
        assert_eq!(&s, "CTGCAATACGC");
        
        let s: String = b.seq().rev().complement().map(|c| c.as_char()).collect();
        assert_eq!(&s, "GCGTATTGCAG");
        
        let s: String = b.seq().rcomplement().map(|c| c.as_char()).collect();
        assert_eq!(&s, "GCGTATTGCAG");
        
        let s: String = b.seq().rcomplement().skip(1).step_by(2).map(|c| c.as_char()).collect();
        assert_eq!(&s, "CTTGA");
        
        let q: Vec<_> = b.qual().collect();
        assert_eq!(&q, &[32, 32, 37, 41, 37, 37, 33, 34, 32, 37, 37]);
        
        let q: Vec<_> = b.qual().rev().collect();
        assert_eq!(&q, &[37, 37, 32, 34, 33, 37, 37, 41, 37, 32, 32]);
        
        let q: Vec<_> = b.qual().rev().skip(1).step_by(3).collect();
        assert_eq!(&q, &[37, 33, 41, 32]);
        
        let mut s = String::new();
        for sq in b.seq_qual() {
            s.push_str(format!("{}:{} ", sq.base(), sq.qual()).as_str())
        }
        assert_eq!(&s, "C:32 T:32 G:37 C:41 A:37 A:37 T:33 A:34 C:32 G:37 C:37 ");
        
        s.clear();
        for sq in b.seq_qual().rcomplement() {
            s.push_str(format!("{}:{} ", sq.base(), sq.qual()).as_str())
        }  
        assert_eq!(&s, "G:37 C:37 G:32 T:34 A:33 T:37 T:37 G:41 C:37 A:32 G:32 ");
        
        s.clear();
        for sq in b.seq_qual().skip(1).step_by(3) {
            s.push_str(format!("{}:{} ", sq.base(), sq.qual()).as_str())
        }
        assert_eq!(&s, "T:32 A:37 A:34 C:37 ");
        
        s.clear();
        for sq in b.seq_qual().rcomplement().skip(1).step_by(3) {
            s.push_str(format!("{}:{} ", sq.base(), sq.qual()).as_str())
        }
        assert_eq!(&s, "C:37 A:33 G:41 G:32 ");
        
        s.clear();
        for sq in b.seq_qual().rcomplement().skip(1).step_by(2) {
            s.push_str(format!("{}:{} ", sq.base(), sq.qual()).as_str())
        }
        assert_eq!(&s, "C:37 T:34 T:37 G:41 A:32 ");        
    }
}
