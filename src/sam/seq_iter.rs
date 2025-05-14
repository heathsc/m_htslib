use std::iter::FusedIterator;

use crate::base::{Base, BaseQual};

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
                if self.n == 1 {
                    self.seq = &[]
                }
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
            let n1 = n + (((self.offset & 2) ^ 2) >> 1) as usize;
            self.seq = &self.seq[..self.seq.len() - (n1 >> 1)];
            self.n -= n;
            self.offset ^= ((n & 1) << 1) as u8;
            self.next_back()
        }
    }
}

impl ExactSizeIterator for SeqIter<'_> {}
impl FusedIterator for SeqIter<'_> {}

pub struct SeqComp<I> {
    it: I,
}

impl<I> SeqComp<I> {
    pub fn new(it: I) -> Self
    where
        I: Sized,
    {
        Self { it }
    }
}

impl<I, T> Iterator for SeqComp<I>
where
    I: Iterator<Item = T>,
    T: SeqComplement,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.it.next().map(|b| b.get_complement())
    }
}

impl<I, T> DoubleEndedIterator for SeqComp<I>
where
    I: DoubleEndedIterator<Item = T>,
    T: SeqComplement,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.it.next_back().map(|b| b.get_complement())
    }
}

impl<I, T> ExactSizeIterator for SeqComp<I>
where
    I: ExactSizeIterator<Item = T>,
    T: SeqComplement,
{
}

impl<I, T> FusedIterator for SeqComp<I>
where
    I: FusedIterator<Item = T>,
    T: SeqComplement,
{
}

pub struct RevSeqComp<I> {
    it: I,
}

impl<I> RevSeqComp<I> {
    pub fn new<T>(it: I) -> Self
    where
        I: Sized + DoubleEndedIterator<Item = T>,
        T: SeqComplement,
    {
        Self { it }
    }
}

impl<I, T> Iterator for RevSeqComp<I>
where
    I: DoubleEndedIterator<Item = T>,
    T: SeqComplement,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.it.next_back().map(|b| b.get_complement())
    }
}

impl<I, T> ExactSizeIterator for RevSeqComp<I>
where
    I: DoubleEndedIterator + ExactSizeIterator<Item = T>,
    T: SeqComplement,
{
}

impl<I, T> FusedIterator for RevSeqComp<I>
where
    I: DoubleEndedIterator + FusedIterator<Item = T>,
    T: SeqComplement,
{
}

pub struct QualIter<'a> {
    qual: &'a [u8],
}

impl<'a> QualIter<'a> {
    pub fn new(qual: &'a [u8]) -> Self {
        Self { qual }
    }
}

impl Iterator for QualIter<'_> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        self.qual.first().map(|x| {
            self.qual = &self.qual[1..];
            *x
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let l = self.qual.len();
        (l, Some(l))
    }

    fn count(self) -> usize {
        self.qual.len()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let n = self.qual.len().min(n);
        self.qual.get(n).map(|x| {
            self.qual = &self.qual[n..];
            *x
        })
    }

    fn last(self) -> Option<Self::Item> {
        self.qual.last().copied()
    }
}

impl DoubleEndedIterator for QualIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.qual.last().map(|x| {
            self.qual = &self.qual[..self.qual.len() - 1];
            *x
        })
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let l = self.qual.len();
        let n = l.min(n);
        if l == n {
            None
        } else {
            let x = self.qual[l - n - 1];
            self.qual = &self.qual[..l - n - 1];
            Some(x)
        }
    }
}

impl ExactSizeIterator for QualIter<'_> {}
impl FusedIterator for QualIter<'_> {}

pub struct SeqQualIter<'a> {
    seq: &'a [u8],
    qual: &'a [u8],
    offset: u8, // If bit 1 is set, next forward is the second base, and if but 2 is set, next reverse is the second base
}

impl<'a> SeqQualIter<'a> {
    pub fn new(seq: &'a [u8], qual: &'a [u8]) -> Self {
        let n = qual.len();
        
        assert_eq!(
            (n + 1) >> 1,
            seq.len(),
            "Mismatch between sequence and quality lengths"
        );
        Self {
            seq,
            qual,
            offset: (((n & 1) ^ 1) << 1) as u8,
        }
    }
}

impl Iterator for SeqQualIter<'_> {
    type Item = BaseQual;

    fn next(&mut self) -> Option<Self::Item> {
        self.seq.first().map(|x| {
            let b = Base::new(if (self.offset & 1) == 0 {
                if self.qual.len() == 1 {
                    self.seq = &[]
                }
                *x >> 4
            } else {
                self.seq = &self.seq[1..];
                *x
            });
            self.offset ^= 1;
            let q = self.qual[0];
            self.qual = &self.qual[1..];
            BaseQual::new(b, q)
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let l = self.qual.len();
        (l, Some(l))
    }

    fn count(self) -> usize {
        self.qual.len()
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        if n == 0 {
            self.next()
        } else if n >= self.qual.len() {
            self.qual = &[];
            self.seq = &[];
            None
        } else {
            let n1 = n + (self.offset & 1) as usize;
            self.seq = &self.seq[n1 >> 1..];
            self.qual = &self.qual[n..];
            self.offset ^= (n & 1) as u8;
            self.next()
        }
    }

    fn last(self) -> Option<Self::Item> {
        self.seq
            .last()
            .map(|x| {
                let b = Base::new(if (self.offset & 2) == 0 { *x >> 4 } else { *x });
                BaseQual::new(b, *self.qual.last().unwrap())
            })
    }
}

impl DoubleEndedIterator for SeqQualIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.seq.last().map(|x| {
            let b = Base::new(if (self.offset & 2) == 0 {
                self.seq = &self.seq[..self.seq.len() - 1];
                *x >> 4
            } else {
                *x
            });
            self.offset ^= 2;
            let l = self.qual.len() - 1;
            let q = self.qual[l];
            self.qual = &self.qual[..l];
            BaseQual::new(b, q)
        })
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        if n == 0 {
            self.next_back()
        } else if n >= self.qual.len() {
            self.qual = &[];
            self.seq = &[];
            None
        } else {
            let n1 = n + (((self.offset & 2) ^ 2) >> 1) as usize;
            self.seq = &self.seq[..self.seq.len() - (n1 >> 1)];
            self.qual = &self.qual[..self.qual.len() - n];
            self.offset ^= ((n & 1) << 1) as u8;
            self.next_back()
        }
    }
}

impl ExactSizeIterator for SeqQualIter<'_> {}
impl FusedIterator for SeqQualIter<'_> {}

pub trait SequenceIter {
    fn complement<T: SeqComplement>(self) -> SeqComp<Self>
    where
        Self: Sized + Iterator<Item = T>,
    {
        SeqComp::new(self)
    }

    fn rcomplement<T: SeqComplement>(self) -> RevSeqComp<Self>
    where
        Self: Sized + DoubleEndedIterator<Item = T>,
    {
        RevSeqComp::new(self)
    }
}


impl<I, T> SequenceIter for I
where
    I: Iterator<Item = T>,
    T: SeqComplement,
{
}

pub trait SeqComplement {
    fn get_complement(&self) -> Self;
}
