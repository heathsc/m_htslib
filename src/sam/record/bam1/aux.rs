use std::{
    collections::HashSet,
    io::{Seek, SeekFrom, Write},
    str::FromStr,
};

use libc::c_int;

use super::{super::BamRec, aux_error::AuxError, aux_iter::BamAuxTag};
use crate::{LeBytes, ParseINumError, sam::BamAuxIter};

/// Represnts a block of tag data that is to be deleted.
/// i is the index of the tag w.r.t to all tags stored in the record
/// offset is the offset in bytes from the start of the data segment for the bam1_t record
/// len is the length in bytes of the section to be deleted.
///
/// If adjacent tags are to be deleted, then the Deletion Blocks are merged. In this case offset is for the
/// start of the merged block, and i is the index of the *last* tag in the block.
struct DeletionBlock {
    i: usize,
    offset: isize,
    len: usize,
}

struct DeletionBlocks {
    v: Vec<DeletionBlock>,
}

impl DeletionBlocks {
    #[inline]
    fn new(n: usize) -> Self {
        Self {
            v: Vec::with_capacity(n),
        }
    }

    fn add_block(&mut self, i: usize, offset: isize, len: usize) {
        let st = |v: &mut Vec<DeletionBlock>| v.push(DeletionBlock { i, offset, len });

        if let Some(p) = self.v.last_mut() {
            if p.i + 1 == i {
                p.i += 1;
                p.len += len;
            } else {
                st(&mut self.v)
            }
        } else {
            st(&mut self.v)
        }
    }
}

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
    pub fn aux_tags<'a>(&'a self) -> BamAuxIter<'a> {
        BamAuxIter::new(self.get_aux_slice())
    }

    pub fn get_tag<'a>(&'a self, tag_id: &str) -> Result<Option<BamAuxTag<'a>>, AuxError> {
        for t in self.aux_tags() {
            let tag = t?;
            if tag.id()? == tag_id {
                return Ok(Some(tag));
            }
        }
        Ok(None)
    }

    /// Delete the tags with the ids in tag_ids. Returns the nnumber of deleted tags on success
    /// Note that if a tag is not found, this does not contitute and error (errors are caused
    /// by the bam1_t structure being corrupt).
    /// Adjacent blocks will be merged for efficiency, and if a block is at the end of the stored tags then
    /// deletion will simply involve altering the l_data field in bam1_t (effectively truncating the data section)
    pub fn del_tags(&mut self, tag_ids: &[&str]) -> Result<usize, AuxError> {
        let (del, n) = self.find_tags_to_delete(tag_ids)?;
        // v contains the start tags and total length of each contiguous block of tags that should be deleted
        // We will do the required moves one by one.
        let mut adj: isize = 0;
        for d in del.v {
            let l_data = self.inner.l_data as isize;
            let l = d.len as isize;
            let off = d.offset - adj;
            let end_off = off + l;
            assert!(end_off <= l_data, "Severe corruption of Bam record");
            if end_off < l_data {
                let sz = l_data - end_off;
                let p = self.inner.data;
                unsafe { p.offset(off).copy_from(p.offset(end_off), sz as usize) }
            }
            self.inner.l_data -= l as c_int;
            adj += l;
        }
        Ok(n)
    }

    #[inline]
    pub fn del_tag(&mut self, tag_id: &str) -> Result<usize, AuxError> {
        self.del_tags(&[tag_id])
    }

    /// Iterate through all tags to find the ones that match, storing the tag data.
    /// If there are multiple tags tso be deleted in adjacent positions then they will be merged.
    fn find_tags_to_delete(&self, tag_ids: &[&str]) -> Result<(DeletionBlocks, usize), AuxError> {
        let mut del = DeletionBlocks::new(tag_ids.len());
        let mut n = 0;
        for (i, t) in self.aux_tags().enumerate() {
            let tag = t?;
            if tag_ids.contains(&tag.id()?) {
                let len = tag.data().len();
                let p = tag.data().as_ptr() as *const i8;
                let off = unsafe { p.offset_from(self.inner.data) };
                del.add_block(i, off, len);
                n += 1;
                if n == tag_ids.len() {
                    // We have found all the tags to be deleted, so no point looking any further
                    break;
                }
            }
        }
        Ok((del, n))
    }

    pub(super) fn parse_aux_tag(
        &mut self,
        s: &[u8],
        hash: &mut HashSet<[u8; 2]>,
    ) -> Result<(), AuxError> {
        if s.len() < 5 {
            Err(AuxError::ShortTag)
        } else if s.len() == 5 && s[3] != b'Z' && s[3] != b'H' {
            Err(AuxError::ZeroLengthTag)
        } else if !(s[0].is_ascii_alphabetic() && s[1].is_ascii_alphanumeric()) {
            Err(AuxError::BadCharsInTagId(s[0], s[1]))
        } else if &[s[2], s[4]] != b"::" {
            Err(AuxError::BadFormat)
        } else if !hash.insert([s[0], s[1]]) {
            // Check if this tag has already been used for this record
            Err(AuxError::DuplicateTagId(s[0] as char, s[1] as char))
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
            let i = parse_i64(p)?;
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
            Err(AuxError::IllegalCharacters)
        } else {
            self.push_z_h_tag(b'Z', s);
            Ok(())
        }
    }

    fn parse_h_tag(&mut self, s: &[u8]) -> Result<(), AuxError> {
        if (s.len() & 1) != 0 {
            Err(AuxError::OddHexDigits)
        } else if s.iter().any(|c| !c.is_ascii_hexdigit()) {
            Err(AuxError::IllegalHexCharacters)
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
        match parse_i64(s)? {
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

pub fn parse_aux_tag<W: Write + Seek>(
    wrt: &mut W,
    s: &[u8],
    hash: &mut HashSet<[u8; 2]>,
) -> Result<(), AuxError> {
    if s.len() < 5 {
        Err(AuxError::ShortTag)
    } else if s.len() == 5 && s[3] != b'Z' && s[3] != b'H' {
        Err(AuxError::ZeroLengthTag)
    } else if !(s[0].is_ascii_alphabetic() && s[1].is_ascii_alphanumeric()) {
        Err(AuxError::BadCharsInTagId(s[0], s[1]))
    } else if &[s[2], s[4]] != b"::" {
        Err(AuxError::BadFormat)
    } else if !hash.insert([s[0], s[1]]) {
        // Check if this tag has already been used for this record
        Err(AuxError::DuplicateTagId(s[0] as char, s[1] as char))
    } else {
        // Copy 2 letter tag ID
        wrt.write_all(&s[..2])?;
        // Parse rest of tag
        parse_tag_body(wrt, &s[3..])
    }
}

fn parse_tag_body<W: Write + Seek>(wrt: &mut W, s: &[u8]) -> Result<(), AuxError> {
    match s[0] {
        // Single character
        b'A' | b'a' | b'C' | b'c' => parse_a_tag(wrt, &s[2..])?,
        // Integer
        b'I' | b'i' => parse_integer(wrt, &s[2..])?,
        // Single precision floating point
        b'f' => copy_num(wrt, b'f', std::str::from_utf8(&s[2..])?.parse::<f32>()?),
        // Double precision floating point (not in the spec, but it is in htslib...)
        b'd' => copy_num(wrt, b'd', std::str::from_utf8(&s[2..])?.parse::<f64>()?),
        // Hex digits
        b'H' => parse_h_tag(wrt, &s[2..])?,
        // Character string
        b'Z' => parse_z_tag(wrt, &s[2..])?,
        // Numeric array
        b'B' => parse_array(wrt, &s[2..])?,
        c => return Err(AuxError::UnknownType(c as char)),
    }
    Ok(())
}

fn parse_array<W: Write + Seek>(wrt: &mut W, s: &[u8]) -> Result<(), AuxError> {
    if s.len() > 1 && s[1] != b',' {
        Err(AuxError::BadFormat)
    } else {
        // We will fill in the types and actual array count later
        // So for now we will jump forward 6 bytes
        wrt.write_all(&[0, 0, 0, 0, 0, 0])?;
        let off = wrt.stream_position()?;

        let (n_elem, tp) = match read_array(wrt, &s[2..], s[0]) {
            Ok(n) => (n, s[0]),
            Err(AuxError::IntegerTooSmall(new_type)) => {
                // Retry with new type. This should not fail (but if it does we will return with an error)
                wrt.seek(SeekFrom::Start(off))?;
                (read_array(wrt, &s[2..], new_type)?, new_type)
            }
            Err(e) => return Err(e),
        };
        wrt.seek(SeekFrom::Start(off - 6))?;
        wrt.write_all(b"B")?;
        copy_num(wrt, tp, n_elem as u32);
        wrt.seek(SeekFrom::End(0))?;
        Ok(())
    }
}

fn read_array<W: Write>(wrt: &mut W, s: &[u8], elem_type: u8) -> Result<usize, AuxError> {
    let res = match elem_type {
        b'c' => read_int_array::<i8, W>(wrt, s),
        b'C' => read_int_array::<u8, W>(wrt, s),
        b's' => read_int_array::<i16, W>(wrt, s),
        b'S' => read_int_array::<u16, W>(wrt, s),
        b'i' => read_int_array::<i32, W>(wrt, s),
        b'I' => read_int_array::<u32, W>(wrt, s),
        b'f' => read_float_array::<f32, W>(wrt, s),
        b'd' => read_float_array::<f64, W>(wrt, s),
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

fn read_int_array<T: LeBytes + TryFrom<i64>, W: Write>(
    wrt: &mut W,
    s: &[u8],
) -> Result<usize, AuxError> {
    let mut n_elem = 0;
    let mut max_val = 0;
    let mut min_val = 0;
    let mut overflow = false;

    for p in s.split(|c| *c == b',') {
        let i = parse_i64(p)?;
        min_val = min_val.min(i);
        max_val = max_val.max(i);
        match i.try_into() {
            Ok(j) => {
                if !overflow {
                    let j: T = j;
                    let _ = wrt.write_all(j.to_le().as_ref());
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

fn read_float_array<T: LeBytes + FromStr, W: Write>(
    wrt: &mut W,
    s: &[u8],
) -> Result<usize, AuxError> {
    let mut n_elem = 0;

    for p in s.split(|c| *c == b',') {
        let i = std::str::from_utf8(p)?
            .parse::<T>()
            .map_err(|_| AuxError::FloatError)?;

        let _ = wrt.write_all(i.to_le().as_ref());
        n_elem += 1;
    }
    Ok(n_elem)
}

fn parse_a_tag<W: Write>(wrt: &mut W, s: &[u8]) -> Result<(), AuxError> {
    if s.len() != 1 || !s[0].is_ascii_graphic() {
        Err(AuxError::BadAFormat)
    } else {
        let _ = wrt.write_all(&[b'A', s[0]]);
        Ok(())
    }
}

fn parse_z_tag<W: Write>(wrt: &mut W, s: &[u8]) -> Result<(), AuxError> {
    if s.iter().any(|c| !(b' '..=b'~').contains(c)) {
        Err(AuxError::IllegalCharacters)
    } else {
        push_z_h_tag(wrt, b'Z', s);
        Ok(())
    }
}

fn parse_h_tag<W: Write>(wrt: &mut W, s: &[u8]) -> Result<(), AuxError> {
    if (s.len() & 1) != 0 {
        Err(AuxError::OddHexDigits)
    } else if s.iter().any(|c| !c.is_ascii_hexdigit()) {
        Err(AuxError::IllegalHexCharacters)
    } else {
        push_z_h_tag(wrt, b'H', s);
        Ok(())
    }
}

fn push_z_h_tag<W: Write>(wrt: &mut W, c: u8, s: &[u8]) {
    let _ = wrt.write_all(&[c]);
    if !s.is_empty() {
        let _ = wrt.write_all(s);
    }
    let _ = wrt.write_all(&[0]);
}

fn parse_integer<W: Write>(wrt: &mut W, s: &[u8]) -> Result<(), AuxError> {
    // We pack an integer into the smallest size that can hold it.
    match parse_i64(s)? {
        i if i < i32::MIN as i64 => return Err(AuxError::IntegerOutOfRange),
        i if i < i16::MIN as i64 => copy_num(wrt, b'i', i as i32),
        i if i < i8::MIN as i64 => copy_num(wrt, b's', i as i16),
        i if i < 0 => wrt.write_all(&[b'c', i as i8 as u8]).unwrap(),
        i if i <= u8::MAX as i64 => wrt.write_all(&[b'c', i as u8]).unwrap(),
        i if i <= u16::MAX as i64 => copy_num(wrt, b'S', i as u16),
        i if i <= u32::MAX as i64 => copy_num(wrt, b'I', i as u32),
        _ => return Err(AuxError::IntegerOutOfRange),
    }
    Ok(())
}

fn copy_num<T: LeBytes, W: Write>(wrt: &mut W, c: u8, x: T) {
    let _ = wrt.write_all(&[c]);
    let _ = wrt.write_all(x.to_le().as_ref());
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

pub(super) fn parse_i64(s: &[u8]) -> Result<i64, ParseINumError> {
    crate::int_utils::parse_i64(s).and_then(|(x, t)| {
        if t.is_empty() {
            Ok(x)
        } else {
            Err(ParseINumError::TrailingGarbage)
        }
    })
}
