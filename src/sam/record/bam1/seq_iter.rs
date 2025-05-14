use std::iter::FusedIterator;

use crate::base::Base;

pub struct SeqIter<'a> {
    seq: &'a [u8],
    n: usize,
    offset: u8, // If bit 1 is set, next forward is the second base, and if but 2 is set, next reverse is the second base
}

impl<'a> SeqIter<'a> {
    pub fn new(seq: &'a [u8], n: usize) -> Self {
        assert_eq!(
            (n + 1) >> 1,
            seq.len(),
            "Mismatch between sequence length and slice"
        );
        Self {
            seq,
            n,
            offset: (((n & 1) ^ 1) << 1) as u8,
        }
    }
}

impl Iterator for SeqIter<'_> {
    type Item = Base;

    fn next(&mut self) -> Option<Self::Item> {
        self.seq.first().map(|x| {
            let b = Base::new(if (self.offset & 1) == 0 {
                *x >> 4
            } else {
                self.seq = &self.seq[1..];
                *x
            });
            self.offset ^= 1;
            self.n -= 1;
            b
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.n, Some(self.n))
    }

    fn count(self) -> usize {
        self.n
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        if n == 0 {
            self.next()
        } else if n >= self.n {
            self.n = 0;
            self.seq = &[];
            None
        } else {
            let n1 = n + (self.offset & 1) as usize;
            self.seq = &self.seq[n1 >> 1..];
            self.n -= n;
            self.offset ^= (n & 1) as u8;
            self.next()
        }
    }

    fn last(self) -> Option<Self::Item> {
        self.seq
            .last()
            .map(|x| Base::new(if (self.offset & 2) == 0 { *x >> 4 } else { *x }))
    }
}

impl DoubleEndedIterator for SeqIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.seq.last().map(|x| {
            let b = Base::new(if (self.offset & 2) == 0 {
                self.seq = &self.seq[..self.seq.len() - 1];
                *x >> 4
            } else {
                *x
            });
            self.offset ^= 2;
            self.n -= 1;
            b
        })
    }
    
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        if n == 0 {
            self.next_back()
        } else if n >= self.n {
            self.n = 0;
            self.seq = &[];
            None
        } else {
            let n1 = n + ((self.offset & 2) ^ 2) as usize;
            self.seq = &self.seq[..self.seq.len() - (n1 >> 1)];
            self.n -= n;
            self.offset ^= ((n & 1) << 1) as u8;
            self.next_back()
        }
    }
}

impl ExactSizeIterator for SeqIter<'_> {}
impl FusedIterator for SeqIter<'_> {}
