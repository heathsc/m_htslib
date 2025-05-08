use std::intrinsics::copy_nonoverlapping;

use super::bam1_t;
use crate::{SamError, sam::SamHdrRaw};

impl bam1_t {
    pub fn parse(&mut self, p: &[u8], hdr: &mut SamHdrRaw) -> Result<(), SamError> {
        self.l_data = 0;

        let mut ix = 0;
        for s in p.split(|c| *c == b'\t') {
            match ix {
                0 => self.parse_qname(s)?,
                1 => self.core.flag = parse_sam_flag(s)?,
                _ => (),
            }
            ix += 1;
        }

        Ok(())
    }


    
    fn parse_qname(&mut self, s: &[u8]) -> Result<(), SamError> {
        let l = s.len();
        if l == 0 {
            Err(SamError::EmptyQueryName)
        } else if l > 254 {
            Err(SamError::QueryTooLong)
        } else {
            self.reserve(l + 4);
            unsafe {
                copy_nonoverlapping(s.as_ptr() as *const i8, self.data, l);
                *self.data.add(l) = 0;
            }
            self.l_data = (l + 1) as i32;
            Ok(())
        }
    }
}

fn parse_sam_flag(s: &[u8]) -> Result<u16,SamError> {
    
    match s.len() {
        0 => Err(SamError::EmptyFlagField),
        1 => Ok((s[0] - b'0') as u16),
        _ => {
            
            // Parse multidigit number, allowing for hex or octal representations
    
            // Get offset and base
            let (ix, base):(usize, u16) = if s[0] == b'0' {
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

mod test {
    use super::*;
    
    #[test]
    fn parse_flag() {
        let x = parse_sam_flag(r"124".as_bytes()).expect("Error parsing decimal");
        assert_eq!(x, 124);
        
        let x = parse_sam_flag(r"0x71".as_bytes()).expect("Error parsing hex");
        assert_eq!(x, 113);
        
        let x = parse_sam_flag(r"071".as_bytes()).expect("Error parsing octal");
        assert_eq!(x, 57);
        
        assert_eq!(parse_sam_flag(r"0193".as_bytes()), Err(SamError::BadFlagFormat));
        
        let x = parse_sam_flag(r"6".as_bytes()).expect("Error parsing decimal");
        assert_eq!(x, 6);
        
        let x = parse_sam_flag(r"0".as_bytes()).expect("Error parsing decimal");
        assert_eq!(x, 0);
    
        let x = parse_sam_flag(r"0x".as_bytes()).expect("Error parsing empty hex");
        assert_eq!(x, 0);
    }
}