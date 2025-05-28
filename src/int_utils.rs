use thiserror::Error;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum ParseINumError {
    // Hard clip ops can only be at the end of cigar strings
    #[error("Empty number")]
    // Only hard clip ops can be between a soft clip and the ends of the cigar
    Empty,
    #[error("Overflow")]
    // Only hard clip ops can be between a soft clip and the ends of the cigar
    Overflow,
    #[error("Trailing garbage")]
    // Only hard clip ops can be between a soft clip and the ends of the cigar
    TrailingGarbage,
}

pub(crate) fn parse_u32(s: &[u8], max: u32) -> Result<(u32, &[u8]), ParseINumError> {
    if s.is_empty() {
        Err(ParseINumError::Empty)
    } else {
        let mut x = 0;
        for (i, c) in s.iter().enumerate() {
            if c.is_ascii_digit() {
                let y = (c - b'0') as u32;
                if x > max / 10 - y {
                    return Err(ParseINumError::Overflow);
                }
                x = x * 10 + y
            } else {
                return Ok((x, &s[i..]));
            }
        }
        Ok((x, &[]))
    }
}

pub(crate) fn parse_u64(s: &[u8], max: u64) -> Result<(u64, &[u8]), ParseINumError> {
    if s.is_empty() {
        Err(ParseINumError::Empty)
    } else {
        let mut x = 0;
        for (i, c) in s.iter().enumerate() {
            if c.is_ascii_digit() {
                let y = (c - b'0') as u64;
                if x > max / 10 - y {
                    return Err(ParseINumError::Overflow);
                }
                x = x * 10 + y
            } else {
                return Ok((x, &s[i..]));
            }
        }
        Ok((x, &[]))
    }
}

pub(crate) fn parse_i64(s: &[u8]) -> Result<(i64, &[u8]), ParseINumError> {
    if s.is_empty() {
        Err(ParseINumError::Empty)
    } else {
        let (s, max, neg) = match s[0] {
            b'-' => (&s[1..], i64::MIN, true),
            b'+' => (&s[1..], i64::MAX, false),
            _ => (s, i64::MAX, false),
        };
        let cut = max / 10;

        let mut x = 0;
        if neg {
            for (i, c) in s.iter().enumerate() {
                if c.is_ascii_digit() {
                    let y = (c - b'0') as i64;
                    if x < cut + y {
                        return Err(ParseINumError::Overflow);
                    }
                    x = x * 10 - y
                } else {
                    return Ok((x, &s[i..]));
                }
            }
        } else {
            for (i, c) in s.iter().enumerate() {
                if c.is_ascii_digit() {
                    let y = (c - b'0') as i64;
                    if x > cut - y {
                        return Err(ParseINumError::Overflow);
                    }
                    x = x * 10 + y
                } else {
                    return Ok((x, &s[i..]));
                }
            }
        }
        Ok((x, &[]))
    }
}
