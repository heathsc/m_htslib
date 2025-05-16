use std::collections::HashSet;

use libc::{c_char, c_int};

use super::{super::BamRec, BAM_FMUNMAP, BAM_FUNMAP, bam1_core_t};
use crate::{
    SamError,
    base::Base,
    hts::HtsPos,
    kstring::KString,
    sam::{SamHdr, cigar_buf::CigarBuf},
};

impl BamRec {
    pub fn parse(
        &mut self,
        p: &[u8],
        hdr: &mut SamHdr,
        ks: &mut KString,
        cb: &mut CigarBuf,
        hash: &mut HashSet<[u8; 2]>,
    ) -> Result<(), SamError> {
        self.inner.l_data = 0;
        self.inner.core = bam1_core_t::default();
        hash.clear();
        
        for (ix, s) in p.split(|c| *c == b'\t').enumerate() {
            match ix {
                0 => self.parse_qname(s)?,
                1 => self.inner.core.flag = parse_sam_flag(s)?,
                2 => self.inner.core.tid = parse_contig(s, hdr, ks)?,
                3 => self.parse_pos(s)?,
                4 => self.inner.core.qual = bytes_to_uint(s, 0xff).map(|x| x as u8)?,
                5 => self.parse_cigar(s, cb)?,
                6 => self.parse_mate_contig(s, hdr, ks)?,
                7 => self.parse_mpos(s)?,
                8 => self.inner.core.isze = std::str::from_utf8(s)?.parse::<HtsPos>()?,
                9 => self.parse_seq(s)?,
                10 => self.parse_qual(s)?,
                _ => self.parse_aux_tag(s, hash)?,
            }
        }

        Ok(())
    }

    const ZEROS: [i8; 4] = [0, 0, 0, 0];

    fn parse_qname(&mut self, s: &[u8]) -> Result<(), SamError> {
        let l = s.len();
        if l == 0 {
            Err(SamError::EmptyQueryName)
        } else if l > 254 {
            Err(SamError::QueryTooLong)
        } else {
            self.inner.copy_data(s);
            let l1 = (4 - ((l + 1) & 3)) & 3;
            self.inner.copy_data(&Self::ZEROS[..=l1]);
            self.inner.core.l_extranul = l1 as u8;
            self.inner.core.l_qname = self.inner.l_data as u16;
            Ok(())
        }
    }

    fn parse_pos(&mut self, s: &[u8]) -> Result<(), SamError> {
        let (pos, tid) = bytes_to_htspos(s, self.inner.core.tid)?;
        self.inner.core.tid = tid;
        self.inner.core.pos = pos;
        if self.inner.core.tid < 0 {
            self.inner.core.flag |= BAM_FUNMAP
        }
        Ok(())
    }

    fn parse_mpos(&mut self, s: &[u8]) -> Result<(), SamError> {
        let (pos, tid) = bytes_to_htspos(s, self.inner.core.mtid)?;
        self.inner.core.mtid = tid;
        self.inner.core.mpos = pos;
        if self.inner.core.mtid < 0 {
            self.inner.core.flag |= BAM_FMUNMAP
        }
        Ok(())
    }

    fn parse_mate_contig(
        &mut self,
        s: &[u8],
        hdr: &mut SamHdr,
        ks: &mut KString,
    ) -> Result<(), SamError> {
        self.inner.core.mtid = if s == b"=" {
            self.inner.core.tid
        } else {
            parse_contig(s, hdr, ks)?
        };
        Ok(())
    }

    fn parse_cigar(&mut self, s: &[u8], cb: &mut CigarBuf) -> Result<(), SamError> {
        if s.is_empty() {
            Err(SamError::EmptyCigarField)
        } else {
            if s[0] == b'*' {
                if self.inner.core.flag & BAM_FUNMAP == 0 {
                    warn!("Mapped query should have Cigar; treated as unmapped");
                    self.inner.core.flag |= BAM_FUNMAP;
                }
            } else {
                let s = std::str::from_utf8(s)?;
                cb.parse_str(s)?;
                let elems = cb.as_elems();
                let n_elem = elems.len();

                if n_elem > u32::MAX as usize {
                    return Err(SamError::TooManyCigarElem);
                }
                self.inner.copy_data(elems);
                self.inner.core.n_cigar = n_elem as u32;
            }
            let cig_ref_len = if self.inner.core.flag & BAM_FUNMAP == 0 {
                1
            } else {
                cb.reference_len().max(1)
            };
            self.inner.core.bin = reg2bin(
                self.inner.core.pos,
                self.inner.core.pos + cig_ref_len as HtsPos,
                14,
                5,
            ) as u16;

            Ok(())
        }
    }

    fn parse_seq(&mut self, s: &[u8]) -> Result<(), SamError> {
        if s == b"*" {
            // Empty sequence
            self.inner.core.l_qseq = 0
        } else {
            // Parse sequence
            let l = s.len();
            if l > i32::MAX as usize {
                return Err(SamError::SeqTooLong);
            }

            // Check Cigar (if present) has same length as sequence
            if let Some(cigar) = self.cigar() {
                if l != cigar.query_len() as usize {
                    return Err(SamError::SeqCigarMismatch);
                }
            }

            self.inner.core.l_qseq = l as i32;
            // Reserve data for sequence
            let nb = (l + 1) >> 1;
            self.inner.reserve(nb);

            // Convert reserve space to &mut[c_char] so that we can work with it safely
            let seq = unsafe {
                std::slice::from_raw_parts_mut(self.inner.data.add(self.inner.l_data as usize), nb)
            };

            self.inner.l_data += nb as i32;

            // Get seq_nt16_table
            let iter = s.chunks_exact(2);
            let r = iter.remainder();

            // Pack sequence into nybbles
            for (s1, p) in iter.zip(seq.iter_mut()) {
                *p = Base::from_u8(s1[0]).combine(&Base::from_u8(s1[1])) as c_char;
            }

            // Do remaining base if seq len is odd
            if let Some(c) = r.first() {
                *seq.last_mut().unwrap() = (Base::from_u8(*c).as_n() << 4) as c_char
            }
        }
        Ok(())
    }

    fn parse_qual(&mut self, s: &[u8]) -> Result<(), SamError> {
        let l = self.inner.core.l_qseq as usize;
        self.inner.reserve(l);

        // Convert reserve space to &mut[c_char] so that we can work with it safely
        let qual = unsafe {
            std::slice::from_raw_parts_mut(self.inner.data.add(self.inner.l_data as usize), l)
        };

        self.inner.l_data += l as i32;

        if s == b"*" {
            qual.fill(-1)
        } else {
            if s.len() != l {
                return Err(SamError::SeqQualMismatch);
            }
            for (sq, q) in s.iter().zip(qual.iter_mut()) {
                *q = (*sq - 33) as c_char
            }
        }
        Ok(())
    }
}

fn parse_contig(s: &[u8], hdr: &mut SamHdr, ks: &mut KString) -> Result<i32, SamError> {
    if s == b"*" {
        Ok(-1)
    } else if hdr.nref() == 0 {
        Err(SamError::NoSqLines)
    } else {
        ks.clear();
        ks.putsn(s)?;
        match hdr.name2tid(ks.as_cstr().unwrap()) {
            Ok(i) => Ok(i as i32),
            Err(SamError::UnknownReference) => {
                warn!("Unrecognized reference name {ks}; treated as unmapped");
                Ok(-1)
            }
            Err(e) => Err(e),
        }
    }
}

fn parse_sam_flag(s: &[u8]) -> Result<u16, SamError> {
    match s.len() {
        0 => Err(SamError::EmptyFlagField),
        1 => Ok((s[0] - b'0') as u16),
        _ => {
            // Parse multidigit number, allowing for hex or octal representations

            // Get offset and base
            let (ix, base): (usize, u16) = if s[0] == b'0' {
                if s[1] == b'x' || s[1] == b'X' {
                    // Hex number. Digits start at s[2]
                    (2, 16)
                } else {
                    // Octal number. Digits start at s[1]
                    (1, 8)
                }
            } else {
                // Decimal. Digits start at s[0]
                (0, 10)
            };

            // The following code is a translation from strtoul.c from gcc

            let cutoff = u16::MAX / base;
            let cutlim = u16::MAX % base;

            s[ix..].iter().try_fold(0, |acc, c| {
                let x = if c.is_ascii_digit() {
                    Ok((c - b'0') as u16)
                } else if c.is_ascii_alphabetic() {
                    Ok(if c.is_ascii_uppercase() {
                        c + 10 - b'A'
                    } else {
                        c + 10 - b'a'
                    } as u16)
                } else {
                    Err(SamError::BadFlagFormat)
                };
                x.and_then(|x| {
                    if x >= base || acc > cutoff || (acc == cutoff && x > cutlim) {
                        Err(SamError::BadFlagFormat)
                    } else {
                        Ok(acc * base + x)
                    }
                })
            })
        }
    }
}

#[inline]
fn bytes_to_htspos(s: &[u8], mut tid: i32) -> Result<(HtsPos, i32), SamError> {
    bytes_to_uint(s, (1u64 << 62) - 1).map(|x| {
        let pos = x as HtsPos - 1;
        if pos < 0 && tid >= 0 {
            warn!("Mapped query cannot have zero coordinate; treated as unmapped");
            tid = -1;
        }
        (pos, tid)
    })
}

fn bytes_to_uint(s: &[u8], max: u64) -> Result<u64, SamError> {
    let x = std::str::from_utf8(s)?.parse::<u64>()?;
    if x > max {
        Err(SamError::ErrorParsingUint)
    } else {
        Ok(x)
    }
}

#[derive(Default)]
pub struct SamParser {
    ks: KString,
    cg: CigarBuf,
    hash: HashSet<[u8; 2]>,
}

impl SamParser {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse(&mut self, b: &mut BamRec, hdr: &mut SamHdr, p: &[u8]) -> Result<(), SamError> {
        b.parse(p, hdr, &mut self.ks, &mut self.cg, &mut self.hash)
    }
}

mod test {
    #[allow(unused)]
    use super::*;

    #[test]
    fn parse_flag() {
        let x = parse_sam_flag(r"124".as_bytes()).expect("Error parsing decimal");
        assert_eq!(x, 124);

        let x = parse_sam_flag(r"0x71".as_bytes()).expect("Error parsing hex");
        assert_eq!(x, 113);

        let x = parse_sam_flag(r"071".as_bytes()).expect("Error parsing octal");
        assert_eq!(x, 57);

        assert_eq!(
            parse_sam_flag(r"0193".as_bytes()),
            Err(SamError::BadFlagFormat)
        );

        let x = parse_sam_flag(r"6".as_bytes()).expect("Error parsing decimal");
        assert_eq!(x, 6);

        let x = parse_sam_flag(r"0".as_bytes()).expect("Error parsing decimal");
        assert_eq!(x, 0);

        let x = parse_sam_flag(r"0x".as_bytes()).expect("Error parsing empty hex");
        assert_eq!(x, 0);
    }
}

fn reg2bin(begin: HtsPos, end: HtsPos, min_shift: c_int, n_lvls: c_int) -> c_int {
    assert!(end > begin);
    let mut s = min_shift;
    let mut t = ((1 << ((n_lvls << 1) + n_lvls)) - 1) / 7;
    let end = end - 1;
    for l in (1..=n_lvls).rev() {
        if begin >> s == end >> s {
            return t + (begin >> s) as c_int;
        }
        s += 3;
        t -= 1 << ((l << 1) + l);
    }
    0
}
