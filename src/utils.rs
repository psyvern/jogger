use std::ops::Range;

pub struct Ranges<I: Iterator<Item = usize>> {
    v: I,
    current: Option<(usize, usize)>,
}

impl<I: Iterator<Item = usize>> Iterator for Ranges<I> {
    type Item = Range<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match (self.v.next(), self.current) {
                (None, None) => break None,
                (None, Some((start, end))) => {
                    self.current = None;
                    break Some(start..end);
                }
                (Some(i), None) => self.current = Some((i, i + 1)),
                (Some(i), Some((start, end))) => {
                    if end == i {
                        self.current = Some((start, i + 1));
                    } else {
                        self.current = Some((i, i + 1));
                        break Some(start..end);
                    }
                }
            }
        }
    }
}

pub trait IteratorExt: Iterator<Item = usize> {
    fn ranges(self) -> Ranges<Self>
    where
        Self: Sized,
    {
        Ranges {
            v: self,
            current: None,
        }
    }
}

impl<I: Iterator<Item = usize>> IteratorExt for I {}
