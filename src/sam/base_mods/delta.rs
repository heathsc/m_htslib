use crate::BaseModsError;

pub(super) type DeltaParseFn = fn(&[u8]) -> Result<(usize, [usize; 2]), BaseModsError>;

pub(super) struct DeltaItr<'a> {
    data: &'a [u8],
    remaining: usize,
    pending: Option<usize>,
    parse: DeltaParseFn,
}

impl<'a> DeltaItr<'a> {
    pub(super) fn new(data: &'a [u8], n_delta: usize, first_delta: usize, reverse: bool) -> Self {
        let parse = if reverse {
            parse_mm_count_rev
        } else {
            parse_mm_count_fwd
        };
        Self {
            data,
            remaining: n_delta,
            pending: Some(first_delta),
            parse,
        }
    }
}

impl<'a> Iterator for DeltaItr<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(x) = self.pending.take() {
            if !self.data.is_empty() {
                let (y, ix) = (self.parse)(self.data).unwrap();
                self.pending = Some(y);
                if ix[0] < self.data.len() {
                    self.data = &self.data[ix[0]..ix[1]]
                } else {
                    self.data = &[];
                }
            }
            assert!(self.remaining > 0);
            self.remaining -= 1;
            Some(x)
        } else {
            assert_eq!(self.remaining, 0);
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

/// Parse a numeric count starting with a comma.  Will panic if v is empty.
/// Returns error if count overflows a usize.  Return tuple with parsed count and start,stop indexes
/// in v of remainder of input vector after removing the parsed entry (or v.len(), v.len() if there
/// is no following entry)
pub(super) fn parse_mm_count_fwd(v: &[u8]) -> Result<(usize, [usize; 2]), BaseModsError> {
    if v[0] != b',' {
        Err(BaseModsError::MissingCommaBeforeCount)
    } else {
        let c = v
            .get(1)
            .and_then(|x| if x.is_ascii_digit() { Some(x) } else { None })
            .ok_or(BaseModsError::MMCountParseError)?;
        let mut ct = (c - b'0') as usize;
        let mut i = 2;
        if v.len() > 2 {
            for c in v[2..].iter() {
                if c.is_ascii_digit() {
                    ct = ct
                        .checked_mul(10)
                        .and_then(|x| x.checked_add((*c - b'0') as usize))
                        .ok_or(BaseModsError::MMCountParseError)?;
                } else {
                    break;
                }
                i += 1
            }
        }
        Ok((ct, [i, v.len()]))
    }
}

/// Parse a numeric count going backwards from the end of the slice amd ending with a comma.
/// Returns tuple with parse count and start,stop indexes of remainder v after removing the parsed
/// entry. Will panic if v is empty. Returns error if count overflows a usize
fn parse_mm_count_rev(v: &[u8]) -> Result<(usize, [usize; 2]), BaseModsError> {
    assert!(!v.is_empty());

    let mut i = v.len() - 1;
    let mut base: usize = 1;
    let c = v[i];
    if !c.is_ascii_digit() {
        return Err(BaseModsError::MMCountParseError);
    }
    let mut x = (c - b'0') as usize;
    for c in v[..i].iter().rev() {
        i -= 1;
        if c.is_ascii_digit() {
            base = base
                .checked_mul(10)
                .ok_or(BaseModsError::MMCountParseError)?;
            x = base
                .checked_mul((*c - b'0') as usize)
                .and_then(|y| x.checked_add(y))
                .ok_or(BaseModsError::MMCountParseError)?;
        } else {
            break;
        }
    }
    if v[i] != b',' {
        Err(BaseModsError::MissingCommaBeforeCount)
    } else {
        Ok((x, [0, i]))
    }
}
