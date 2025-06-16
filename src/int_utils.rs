use std::{
    cmp::Ordering,
    ops::{Add, Div, Mul, Rem, Sub},
};

use thiserror::Error;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum ParseINumError {
    #[error("Empty number")]
    Empty,
    #[error("Missing Exponent")]
    MissingExponent,
    #[error("Overflow")]
    Overflow,
    #[error("Trailing garbage")]
    TrailingGarbage,
}

pub(crate) fn parse_uint<T>(s: &[u8], max: T) -> Result<(T, usize), ParseINumError>
where
    T: Copy
        + From<u8>
        + PartialOrd
        + Div<Output = T>
        + Mul<Output = T>
        + Sub<Output = T>
        + Add<Output = T>
        + Rem<Output = T>,
{
    let ten: T = 10.into();
    let cut = max / ten;
    let lim = max % ten;

    if s.is_empty() {
        Err(ParseINumError::Empty)
    } else {
        let mut x: T = 0.into();
        for (i, c) in s.iter().enumerate() {
            if c.is_ascii_digit() {
                let y: T = (c - b'0').into();
                if x > cut || (x == cut && y > lim) {
                    return Err(ParseINumError::Overflow);
                }
                x = x * ten + y
            } else {
                return Ok((x, i));
            }
        }
        Ok((x, s.len()))
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
        let lim = max % 10;

        let mut x = 0;
        if neg {
            for (i, c) in s.iter().enumerate() {
                if c.is_ascii_digit() {
                    let y = (c - b'0') as i64;
                    if x < cut || (x == cut && y > lim) {
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
                    if x > cut || (x == cut && y > lim) {
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

/// Clone of hts_parse_decimal() from htslib (which is a private function)
/// with the addition of checks for overflow
///
/// Parse coordinate which can:
///
///  - Contain commas
///  - have a leading +/-
///  - be written as in E form (i.e., 1.4E6)
///  - be followed by K/M/G
pub(crate) fn parse_decimal(s: &[u8], no_sign: bool) -> Result<(i64, &[u8]), ParseINumError> {
    // Skip leading whitespace
    let s = skip_space(s);
    
    // Get sign
    let (negative, i) = if no_sign { (false, 0) } else { get_sign(s) };
    
    // What we have left to work with...
    let s = &s[i..];

    // Get initial (integral) part of number
    let (x, mut i, _) = get_num(s, false, 0)?;

    // Get fractional part (if present)
    let (x, i1, j) = get_frac(&s[i..], x);
    i += i1;

    if i == 0 {
        // No digits encountered
        Err(ParseINumError::Empty)
    } else {
        // Check for E format
        let (ex, i1) = parse_exp(&s[i..], -j)?;
        i += i1;

        // Parse suffix
        let (ex, i1) = parse_suffix(&s[i..], ex)?;
        i += i1;

        // Adjust for exponent if nexeaary
        let x = adj_for_exp(x, ex)?;

        // Adjust for sign
        let x = if negative { -x } else { x };

        Ok((x, &s[i..]))
    }
}

fn adj_for_exp(x: i64, ex: i32) -> Result<i64, ParseINumError> {
    Ok(match ex.cmp(&0) {
        Ordering::Greater => {
            let z = 10i64
                .checked_pow(ex as u32)
                .ok_or(ParseINumError::Overflow)?;
            x.checked_mul(z).ok_or(ParseINumError::Overflow)?
        }
        Ordering::Less => 10i64.checked_pow(-ex as u32).map(|z| x / z).unwrap_or(0),
        _ => x,
    })
}

fn parse_suffix(s: &[u8], ex: i32) -> Result<(i32, usize), ParseINumError> {
    let m = if let Some(c) = s.first() {
        match *c {
            b'K' | b'k' => 3,
            b'M' | b'm' => 6,
            b'G' | b'g' => 9,
            _ => 0,
        }
    } else {
        0
    };
    if m > 0 {
        Ok((ex.checked_add(m).ok_or(ParseINumError::Overflow)?, 1))
    } else {
        Ok((ex, 0))
    }
}

fn parse_exp(s: &[u8], ex: i32) -> Result<(i32, usize), ParseINumError> {
    if matches!(s.first(), Some(b'E') | Some(b'e')) {
        let (neg, j) = get_sign(&s[1..]);
        let (k, i1) = parse_uint(&s[1 + j..], i32::MAX)?;
        let ex = if neg {
            ex.saturating_sub(k)
        } else {
            ex.checked_add(k).ok_or(ParseINumError::Overflow)?
        };
        Ok((ex, i1 + 1 + j))
    } else {
        Ok((ex, 0))
    }
}

const I64CUT: i64 = i64::MAX / 10;
const I64LIM: i64 = i64::MAX % 10;

fn get_frac(s: &[u8], x_init: i64) -> (i64, usize, i32) {
    s.first()
        .map(|c| {
            if *c == b'.' {
                // We are ignoring overflow here, so we can safely unwrap the result
                let (x, i, j) = get_num(&s[1..], true, x_init).unwrap();
                (x, i + 1, j)
            } else {
                (x_init, 0, 0)
            }
        })
        .unwrap_or((x_init, 0, 0))
}

fn get_num(
    s: &[u8],
    ignore_overflow: bool,
    x_init: i64,
) -> Result<(i64, usize, i32), ParseINumError> {
    let mut overflow = false;
    let (x, i, j) = s
        .iter()
        .enumerate()
        .filter(|(_, c)| **c != b',')
        .take_while(|(_, c)| c.is_ascii_digit())
        .fold((x_init, 0, 0), |(x, _, j), (i, c)| {
            let d = (c - b'0') as i64;
            if x > I64CUT || (x == I64CUT && d > I64LIM) {
                overflow = true
            }
            if overflow {
                (x, i + 1, j)
            } else {
                (x * 10 + d, i + 1, j + 1)
            }
        });
    if overflow && !ignore_overflow {
        Err(ParseINumError::Overflow)
    } else {
        Ok((x, i, j))
    }
}

#[inline]
pub(crate) fn skip_space(s: &[u8]) -> &[u8] {
    s.iter()
        .position(|c| !c.is_ascii_whitespace())
        .map(|i| &s[i..])
        .unwrap_or(&[])
}

#[inline]
fn get_sign(s: &[u8]) -> (bool, usize) {
    s.first()
        .map(|c| match c {
            b'+' => (false, 1),
            b'-' => (true, 1),
            _ => (false, 0),
        })
        .unwrap_or((false, 0))
}

#[cfg(test)]
mod tests {
    #![allow(unused)]

    use super::*;

    #[test]
    fn test_parse_decimal() {
        let (i, s) = parse_decimal(b"348695", false).unwrap();
        assert_eq!(i, 348695);
        let (i, s) = parse_decimal(b"2,348,695", false).unwrap();
        assert_eq!(i, 2348695);
        assert_eq!(s, &[]);
        let (i, _) = parse_decimal(b"1.4M", false).unwrap();
        assert_eq!(i, 1400000);
        let (i, _s) = parse_decimal(b"1.4212e3", false).unwrap();
        assert_eq!(i, 1421);
        let (i, _s) = parse_decimal(b"-1.42E2K", false).unwrap();
        assert_eq!(i, -142000);
        let (i, s) = parse_decimal(b"7.2E-2M,432", false).unwrap();
        assert_eq!(i, 72000);
        assert_eq!(s, b",432");
    }
}
