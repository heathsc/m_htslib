use std::str::FromStr;

use super::{aux_error::AuxError, super::BamRec};
use crate::{LeBytes, sam::BamAuxIter};

impl BamRec {
    fn get_aux_slice(&self) -> &[u8] {
        let b = &self.inner;
        let core = &b.core;
        let off = ((core.n_cigar as usize) << 2)
            + core.l_qname as usize
            + core.l_qseq as usize
            + ((core.l_qseq + 1) >> 1) as usize;
        
        assert!(off <= b.l_data as usize, "Corrupt BAM record");
        let sz = b.l_data as usize - off;
        self.make_data_slice(off, sz)
    }
    
    #[inline]
    pub fn aux(&self) -> BamAuxIter {
        BamAuxIter::new(self.get_aux_slice())
    }
    
    pub(super) fn parse_aux_tag(&mut self, s: &[u8]) -> Result<(), AuxError> {
        if s.len() < 5 {
            Err(AuxError::ShortTag)
        } else if s.len() == 5 && s[3] != b'Z' && s[3] != b'H' {
            Err(AuxError::ZeroLengthTag)
        } else if &[s[2], s[4]] != b"::" {
            Err(AuxError::BadFormat)
        } else {
            // Copy 2 letter tag ID
            self.inner.copy_data(&s[..2]);
            // Parse rest of tag
            self.parse_tag_body(&s[3..])
        }
    }

    fn parse_tag_body(&mut self, s: &[u8]) -> Result<(), AuxError> {
        match s[0] {
            // Single character
            b'A' | b'a' | b'C' | b'c' => self.parse_a_tag(&s[2..])?,
            // Integer
            b'I' | b'i' => self.parse_integer(&s[2..])?,
            // Single precision floating point
            b'f' => self.copy_num(b'f', std::str::from_utf8(&s[2..])?.parse::<f32>()?),
            // Double precision floating point (not in the spec, but it is in htslib...)
            b'd' => self.copy_num(b'd', std::str::from_utf8(&s[2..])?.parse::<f64>()?),
            // Hex digits
            b'H' => self.parse_h_tag(&s[2..])?,
            // Character string
            b'Z' => self.parse_z_tag(&s[2..])?,
            // Numeric array
            b'B' => self.parse_array(&s[2..])?,
            c => return Err(AuxError::UnknownType(c as char)),
        }
        Ok(())
    }

    fn parse_array(&mut self, s: &[u8]) -> Result<(), AuxError> {
        if s.len() > 1 && s[1] != b',' {
            Err(AuxError::BadFormat)
        } else {
            let off = self.inner.l_data;
            self.inner.reserve(6);

            // We will fill in the types and actual array count later
            self.inner.l_data += 6;

            let (n_elem, tp) = match self.read_array(&s[2..], s[0]) {
                Ok(n) => (n, s[0]),
                Err(AuxError::IntegerTooSmall(new_type)) => {
                    // Retry with new type. This should not fail (but if it does we will return with an error)
                    self.inner.l_data = off + 6;
                    (self.read_array(&s[2..], new_type)?, new_type)
                }
                Err(e) => return Err(e),
            };

            let last = self.inner.l_data;
            self.inner.l_data = off;
            self.inner.push_char(b'B');
            self.copy_num(tp, n_elem as u32);
            self.inner.l_data = last;
            Ok(())
        }
    }

    fn read_array(&mut self, s: &[u8], elem_type: u8) -> Result<usize, AuxError> {
        let res = match elem_type {
            b'c' => self.read_int_array::<i8>(s),
            b'C' => self.read_int_array::<u8>(s),
            b's' => self.read_int_array::<i16>(s),
            b'S' => self.read_int_array::<u16>(s),
            b'i' => self.read_int_array::<i32>(s),
            b'I' => self.read_int_array::<u32>(s),
            b'f' => self.read_float_array::<f32>(s),
            b'd' => self.read_float_array::<f64>(s),
            _ => Err(AuxError::UnknownArrayType(elem_type as char)),
        };

        // CHeck for overflow
        if let Err(AuxError::IntegerOverflow((min_val, max_val))) = res {
            // If we did overflow (this can only occur with an integer type), find the
            // smallest type that can hold all values and return that
            let new_type = find_best_type(min_val, max_val)?;
            Err(AuxError::IntegerTooSmall(new_type))
        } else {
            let n_elem = res?;
            Ok(n_elem)
        }
    }

    fn read_int_array<T: LeBytes + TryFrom<i64>>(&mut self, s: &[u8]) -> Result<usize, AuxError> {
        let mut n_elem = 0;
        let mut max_val = 0;
        let mut min_val = 0;
        let mut overflow = false;

        for p in s.split(|c| *c == b',') {
            let i = std::str::from_utf8(p)?.parse::<i64>()?;
            min_val = min_val.min(i);
            max_val = max_val.max(i);
            match i.try_into() {
                Ok(j) => {
                    if !overflow {
                        let j: T = j;
                        self.inner.copy_data(j.to_le().as_ref());
                        n_elem += 1;
                    }
                }
                Err(_) => overflow = true,
            }
        }
        if overflow {
            Err(AuxError::IntegerOverflow((min_val, max_val)))
        } else {
            Ok(n_elem)
        }
    }

    fn read_float_array<T: LeBytes + FromStr>(&mut self, s: &[u8]) -> Result<usize, AuxError> {
        let mut n_elem = 0;

        for p in s.split(|c| *c == b',') {
            let i = std::str::from_utf8(p)?
                .parse::<T>()
                .map_err(|_| AuxError::FloatError)?;

            self.inner.copy_data(i.to_le().as_ref());
            n_elem += 1;
        }
        Ok(n_elem)
    }

    fn parse_a_tag(&mut self, s: &[u8]) -> Result<(), AuxError> {
        if s.len() != 1 || !s[0].is_ascii_graphic() {
            Err(AuxError::BadAFormat)
        } else {
            self.inner.copy_data(&[b'A', s[0]]);
            Ok(())
        }
    }

    fn parse_z_tag(&mut self, s: &[u8]) -> Result<(), AuxError> {
        if s.iter().any(|c| !(b' '..=b'~').contains(c)) {
            Err(AuxError::IllegalZCharacters)
        } else {
            self.push_z_h_tag(b'Z', s);
            Ok(())
        }
    }

    fn parse_h_tag(&mut self, s: &[u8]) -> Result<(), AuxError> {
        if (s.len() & 1) != 0 {
            Err(AuxError::OddHexDigits)
        } else if s.iter().any(|c| !c.is_ascii_hexdigit()) {
            Err(AuxError::IllegalHCharacters)
        } else {
            self.push_z_h_tag(b'H', s);
            Ok(())
        }
    }

    fn push_z_h_tag(&mut self, c: u8, s: &[u8]) {
        self.inner.push_char(c);
        if !s.is_empty() {
            self.inner.copy_data(s);
        }
        self.inner.push_char(0);
    }

    fn parse_integer(&mut self, s: &[u8]) -> Result<(), AuxError> {
        // We pack an integer into the smallest size that can hold it.
        match std::str::from_utf8(s)?.parse::<i64>()? {
            i if i < i32::MIN as i64 => return Err(AuxError::IntegerOutOfRange),
            i if i < i16::MIN as i64 => self.copy_num(b'i', i as i32),
            i if i < i8::MIN as i64 => self.copy_num(b's', i as i16),
            i if i < 0 => self.inner.copy_data(&[b'c' as i8, i as i8]),
            i if i <= u8::MAX as i64 => self.inner.copy_data(&[b'C', i as u8]),
            i if i <= u16::MAX as i64 => self.copy_num(b'S', i as u16),
            i if i <= u32::MAX as i64 => self.copy_num(b'I', i as u32),
            _ => return Err(AuxError::IntegerOutOfRange),
        }
        Ok(())
    }

    fn copy_num<T: LeBytes>(&mut self, c: u8, x: T) {
        self.inner.push_char(c);
        self.inner.copy_data(x.to_le().as_ref());
    }
}

fn find_best_type(min_val: i64, max_val: i64) -> Result<u8, AuxError> {
    if min_val < 0 {
        if min_val >= i8::MIN as i64 && max_val <= i8::MAX as i64 {
            Ok(b'c')
        } else if min_val >= i16::MIN as i64 && max_val <= i16::MAX as i64 {
            Ok(b's')
        } else if min_val >= i32::MIN as i64 && max_val <= i32::MAX as i64 {
            Ok(b'i')
        } else {
            Err(AuxError::IntegerOutOfRange)
        }
    } else if max_val <= u8::MAX as i64 {
        Ok(b'C')
    } else if max_val <= u16::MAX as i64 {
        Ok(b'S')
    } else if max_val <= u32::MAX as i64 {
        Ok(b'I')
    } else {
        Err(AuxError::IntegerOutOfRange)
    }
}
